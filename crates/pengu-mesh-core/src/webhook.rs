use serde::Serialize;
use std::net::IpAddr;
use std::time::Duration;
use url::{Host, Url};

/// Webhook validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebhookError {
    InvalidUrl,
    InvalidScheme,
    SsrfBlocked,
}

impl std::fmt::Display for WebhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid webhook URL"),
            Self::InvalidScheme => write!(f, "only http and https schemes are allowed"),
            Self::SsrfBlocked => write!(
                f,
                "webhook URL resolves to a blocked local, private, or loopback host"
            ),
        }
    }
}

impl std::error::Error for WebhookError {}

/// Configuration for webhook delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebhookConfig {
    pub url: String,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

impl WebhookConfig {
    pub(crate) fn new(url: String) -> Self {
        Self {
            url,
            timeout_ms: 5_000,
            max_retries: 2,
        }
    }
}

/// JSON payload sent to the webhook endpoint.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct WebhookPayload {
    pub event: String,
    pub task_id: String,
    pub state: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Outcome of a webhook delivery attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebhookDeliveryResult {
    pub status_code: Option<u16>,
    pub success: bool,
    pub attempts: u32,
    pub error: Option<String>,
}

/// Validate a webhook URL, rejecting non-http(s) schemes and local/private targets.
pub(crate) fn validate_webhook_url(url: &str) -> Result<(), WebhookError> {
    let parsed = Url::parse(url).map_err(|_| WebhookError::InvalidUrl)?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err(WebhookError::InvalidScheme),
    }

    let host = parsed.host().ok_or(WebhookError::InvalidUrl)?;
    if is_blocked_host(&host) {
        return Err(WebhookError::SsrfBlocked);
    }

    Ok(())
}

/// Deliver a webhook payload with best-effort retry logic.
///
/// The function attempts up to `1 + config.max_retries` POSTs. On a 2xx
/// response it returns immediately. Non-2xx or transport errors are retried
/// until the budget is exhausted.
pub(crate) fn deliver_webhook(
    config: &WebhookConfig,
    payload: &WebhookPayload,
) -> WebhookDeliveryResult {
    deliver_webhook_inner(config, payload, true)
}

fn is_blocked_host(host: &Host<&str>) -> bool {
    match host {
        Host::Ipv4(address) => is_blocked_ip((*address).into()),
        Host::Ipv6(address) => is_blocked_ip((*address).into()),
        Host::Domain(domain) => {
            let domain = domain.to_ascii_lowercase();
            domain == "localhost" || domain.ends_with(".localhost")
        }
    }
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_unspecified()
        }
    }
}

fn deliver_webhook_inner(
    config: &WebhookConfig,
    payload: &WebhookPayload,
    validate_url: bool,
) -> WebhookDeliveryResult {
    if validate_url {
        if let Err(error) = validate_webhook_url(&config.url) {
            return WebhookDeliveryResult {
                status_code: None,
                success: false,
                attempts: 0,
                error: Some(error.to_string()),
            };
        }
    }

    let body = match serde_json::to_vec(payload) {
        Ok(body) => body,
        Err(error) => {
            return WebhookDeliveryResult {
                status_code: None,
                success: false,
                attempts: 0,
                error: Some(format!("failed to serialize payload: {error}")),
            };
        }
    };

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_millis(config.timeout_ms)))
            .build(),
    );

    let total_attempts = 1 + config.max_retries;
    let mut last_status = None;
    let mut last_error = None;

    for attempt in 1..=total_attempts {
        let request = agent
            .post(&config.url)
            .content_type("application/json")
            .header("X-Pengu-Mesh-Event", payload.event.as_str())
            .header("X-Pengu-Mesh-Task-ID", payload.task_id.as_str());

        match request.send(body.as_slice()) {
            Ok(response) => {
                let status = response.status().as_u16();
                if (200..300).contains(&status) {
                    return WebhookDeliveryResult {
                        status_code: Some(status),
                        success: true,
                        attempts: attempt,
                        error: None,
                    };
                }

                last_status = Some(status);
                last_error = Some(format!("non-2xx status: {status}"));
            }
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }
    }

    WebhookDeliveryResult {
        status_code: last_status,
        success: false,
        attempts: total_attempts,
        error: last_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;

    #[derive(Debug, Clone)]
    struct TestResponse {
        status_code: u16,
        body: String,
        delay_ms: u64,
    }

    #[derive(Debug)]
    struct TestServer {
        url: String,
        requests: mpsc::Receiver<String>,
        handle: thread::JoinHandle<()>,
    }

    impl TestServer {
        fn join(self) -> Vec<String> {
            self.handle.join().expect("test server thread should join");
            self.requests.try_iter().collect()
        }
    }

    #[test]
    fn valid_https_url() {
        assert!(validate_webhook_url("https://example.com/hook").is_ok());
    }

    #[test]
    fn valid_http_url() {
        assert!(validate_webhook_url("http://example.com/hook").is_ok());
    }

    #[test]
    fn rejects_ftp_scheme() {
        let err = validate_webhook_url("ftp://example.com/hook").unwrap_err();
        assert_eq!(err, WebhookError::InvalidScheme);
    }

    #[test]
    fn rejects_invalid_url() {
        let err = validate_webhook_url("not-a-url").unwrap_err();
        assert_eq!(err, WebhookError::InvalidUrl);
    }

    #[test]
    fn rejects_missing_host() {
        let err = validate_webhook_url("https://").unwrap_err();
        assert_eq!(err, WebhookError::InvalidUrl);
    }

    #[test]
    fn rejects_loopback_127() {
        let err = validate_webhook_url("http://127.0.0.1/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_private_10() {
        let err = validate_webhook_url("http://10.0.0.1/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_private_172_16() {
        let err = validate_webhook_url("http://172.16.0.1/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_private_192_168() {
        let err = validate_webhook_url("http://192.168.1.1/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_link_local_ipv4() {
        let err = validate_webhook_url("http://169.254.0.1/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_localhost_hostname() {
        let err = validate_webhook_url("http://localhost/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_localhost_subdomain() {
        let err = validate_webhook_url("http://api.localhost/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_ipv6_loopback() {
        let err = validate_webhook_url("http://[::1]/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn rejects_ipv6_unique_local() {
        let err = validate_webhook_url("http://[fd00::1]/hook").unwrap_err();
        assert_eq!(err, WebhookError::SsrfBlocked);
    }

    #[test]
    fn allows_172_outside_private_range() {
        assert!(validate_webhook_url("http://172.32.0.1/hook").is_ok());
    }

    #[test]
    fn payload_serializes_without_optional_data() {
        let payload = test_payload(None);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("task.completed"));
        assert!(!json.contains("\"data\""));
    }

    #[test]
    fn config_defaults() {
        let cfg = WebhookConfig::new("https://example.com".into());
        assert_eq!(cfg.timeout_ms, 5_000);
        assert_eq!(cfg.max_retries, 2);
    }

    #[test]
    fn deliver_rejects_ssrf_targets_without_attempts() {
        let result = deliver_webhook(
            &WebhookConfig::new("http://127.0.0.1/hook".into()),
            &test_payload(None),
        );

        assert_eq!(
            result,
            WebhookDeliveryResult {
                status_code: None,
                success: false,
                attempts: 0,
                error: Some(
                    "webhook URL resolves to a blocked local, private, or loopback host".into(),
                ),
            }
        );
    }

    #[test]
    fn deliver_webhook_retries_until_success() {
        let server = spawn_test_server(vec![
            TestResponse {
                status_code: 500,
                body: "retry".into(),
                delay_ms: 0,
            },
            TestResponse {
                status_code: 204,
                body: String::new(),
                delay_ms: 0,
            },
        ]);
        let config = WebhookConfig {
            url: server.url.clone(),
            timeout_ms: 1_000,
            max_retries: 2,
        };
        let payload = test_payload(Some(json!({ "attempt": "final" })));

        let result = deliver_webhook_inner(&config, &payload, false);
        let requests = server.join();

        assert_eq!(
            result,
            WebhookDeliveryResult {
                status_code: Some(204),
                success: true,
                attempts: 2,
                error: None,
            }
        );
        assert_eq!(requests.len(), 2);
        for request in requests {
            assert!(request.starts_with("POST /hook HTTP/1.1\r\n"));
            assert!(request.contains("content-type: application/json\r\n"));
            assert!(request.contains("x-pengu-mesh-event: task.completed\r\n"));
            assert!(request.contains("x-pengu-mesh-task-id: abc-123\r\n"));
            assert!(request.contains("\"attempt\":\"final\""));
        }
    }

    #[test]
    fn deliver_webhook_stops_after_retry_budget() {
        let server = spawn_test_server(vec![
            TestResponse {
                status_code: 500,
                body: "retry".into(),
                delay_ms: 0,
            },
            TestResponse {
                status_code: 502,
                body: "retry".into(),
                delay_ms: 0,
            },
        ]);
        let config = WebhookConfig {
            url: server.url.clone(),
            timeout_ms: 1_000,
            max_retries: 1,
        };

        let result = deliver_webhook_inner(&config, &test_payload(None), false);
        let requests = server.join();

        assert_eq!(requests.len(), 2);
        assert_eq!(result.status_code, Some(502));
        assert!(!result.success);
        assert_eq!(result.attempts, 2);
        assert_eq!(result.error.as_deref(), Some("non-2xx status: 502"));
    }

    #[test]
    fn deliver_webhook_reports_timeout() {
        let server = spawn_test_server(vec![TestResponse {
            status_code: 204,
            body: String::new(),
            delay_ms: 150,
        }]);
        let config = WebhookConfig {
            url: server.url.clone(),
            timeout_ms: 25,
            max_retries: 0,
        };

        let result = deliver_webhook_inner(&config, &test_payload(None), false);
        let requests = server.join();

        assert_eq!(requests.len(), 1);
        assert!(!result.success);
        assert_eq!(result.attempts, 1);
        assert!(result.status_code.is_none());
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|error| error.contains("timeout")),
            "expected timeout error, got {:?}",
            result.error
        );
    }

    fn test_payload(data: Option<serde_json::Value>) -> WebhookPayload {
        WebhookPayload {
            event: "task.completed".into(),
            task_id: "abc-123".into(),
            state: "done".into(),
            timestamp: "2025-01-01T00:00:00Z".into(),
            data,
        }
    }

    fn spawn_test_server(responses: Vec<TestResponse>) -> TestServer {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind test listener on loopback");
        let addr = listener.local_addr().expect("resolve local test address");
        listener
            .set_nonblocking(false)
            .expect("configure test listener");

        let (request_tx, request_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept test request");
                let request = read_request(&mut stream);
                request_tx.send(request).expect("send captured request");
                if response.delay_ms > 0 {
                    thread::sleep(Duration::from_millis(response.delay_ms));
                }
                let _ = write_response(&mut stream, &response);
            }
        });

        TestServer {
            url: format!("http://{addr}/hook"),
            requests: request_rx,
            handle,
        }
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set request read timeout");

        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).expect("read request chunk");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);

            if let Some(headers_end) = find_headers_end(&buffer) {
                let body_start = headers_end + 4;
                let content_length = parse_content_length(&buffer[..headers_end]);
                if buffer.len() >= body_start + content_length {
                    break;
                }
            }
        }

        String::from_utf8(buffer).expect("request should be valid utf-8 in tests")
    }

    fn find_headers_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn parse_content_length(headers: &[u8]) -> usize {
        let headers = std::str::from_utf8(headers).expect("headers should be valid utf-8");
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    fn write_response(stream: &mut TcpStream, response: &TestResponse) -> std::io::Result<()> {
        let status_text = match response.status_code {
            200 => "OK",
            204 => "No Content",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            code => panic!("unsupported test status code {code}"),
        };
        let payload = response.body.as_bytes();
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.status_code,
            status_text,
            payload.len()
        )?;
        stream.write_all(payload)?;
        stream.flush()?;
        Ok(())
    }
}
