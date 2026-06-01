use std::fmt;
use std::io;
use std::time::Duration;

/// Configuration for the HTTP reverse proxy.
#[derive(Debug, Clone)]
pub(crate) struct ProxyConfig {
    /// The base URL of the upstream target (for example `http://localhost:9222`).
    pub target_url: String,
    /// Request timeout in milliseconds. Defaults to 30_000.
    pub timeout_ms: u64,
    /// When `true`, the original `Host` header is forwarded to the upstream.
    pub preserve_host: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            target_url: String::new(),
            timeout_ms: 30_000,
            preserve_host: false,
        }
    }
}

/// A simplified HTTP request to be forwarded through the proxy.
#[derive(Debug, Clone)]
pub(crate) struct ProxyRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// The response returned from the upstream.
#[derive(Debug, Clone)]
pub(crate) struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Errors that can occur during proxying.
#[derive(Debug)]
pub(crate) enum ProxyError {
    /// The target URL could not be parsed or is otherwise invalid.
    InvalidTarget(String),
    /// The forwarded request could not be represented as a valid upstream request.
    InvalidRequest(String),
    /// A TCP connection to the upstream could not be established.
    ConnectionFailed(String),
    /// The upstream did not respond within the configured timeout.
    Timeout,
    /// The upstream returned data that could not be read or otherwise failed.
    UpstreamError(String),
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget(msg) => write!(f, "invalid target: {msg}"),
            Self::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
            Self::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            Self::Timeout => write!(f, "upstream request timed out"),
            Self::UpstreamError(msg) => write!(f, "upstream error: {msg}"),
        }
    }
}

impl std::error::Error for ProxyError {}

/// Build the full upstream URL by joining the target base with the request path.
///
/// Returns [`ProxyError::InvalidTarget`] when the configured `target_url` is
/// not a valid URL or the path cannot be joined.
pub(crate) fn build_proxy_url(config: &ProxyConfig, path: &str) -> Result<String, ProxyError> {
    let base = url::Url::parse(&config.target_url)
        .map_err(|error| ProxyError::InvalidTarget(error.to_string()))?;

    let joined = base
        .join(path)
        .map_err(|error| ProxyError::InvalidTarget(error.to_string()))?;

    Ok(joined.to_string())
}

/// Detect whether the given headers represent a WebSocket upgrade request.
///
/// Checks for `Connection: upgrade` and `Upgrade: websocket` case-insensitively.
pub(crate) fn is_websocket_upgrade(headers: &[(String, String)]) -> bool {
    let has_upgrade_connection = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("connection")
            && value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
    });

    let has_websocket_upgrade = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("upgrade") && value.trim().eq_ignore_ascii_case("websocket")
    });

    has_upgrade_connection && has_websocket_upgrade
}

const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Forward an HTTP request through the proxy to the upstream target.
///
/// The helper disables redirect following and treats upstream `3xx`, `4xx`,
/// and `5xx` responses as ordinary proxy responses instead of local errors.
pub(crate) fn forward_request(
    config: &ProxyConfig,
    request: &ProxyRequest,
) -> Result<ProxyResponse, ProxyError> {
    let url = build_proxy_url(config, &request.path)?;
    let connection_tokens = connection_header_tokens(&request.headers);

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_millis(config.timeout_ms)))
            .build(),
    );

    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(url.as_str());

    for (name, value) in &request.headers {
        if is_hop_by_hop_header(name, &connection_tokens) {
            continue;
        }
        if !config.preserve_host && name.eq_ignore_ascii_case("host") {
            continue;
        }
        builder = builder.header(name.as_str(), value.as_str());
    }

    let response = match request.body.as_deref() {
        Some(body) => {
            let upstream_request = builder
                .body(body)
                .map_err(|error| ProxyError::InvalidRequest(error.to_string()))?;
            agent.run(upstream_request)
        }
        None => {
            let upstream_request = builder
                .body(())
                .map_err(|error| ProxyError::InvalidRequest(error.to_string()))?;
            agent.run(upstream_request)
        }
    }
    .map_err(classify_ureq_error)?;

    into_proxy_response(response)
}

fn into_proxy_response(
    response: ureq::http::Response<ureq::Body>,
) -> Result<ProxyResponse, ProxyError> {
    let response_connection_tokens = header_map_connection_tokens(response.headers());
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            if is_hop_by_hop_header(name.as_str(), &response_connection_tokens) {
                return None;
            }
            Some((name.as_str().to_string(), header_value_to_string(value)))
        })
        .collect();

    let status = response.status().as_u16();
    let body = response.into_body().read_to_vec().map_err(|error| {
        ProxyError::UpstreamError(format!("failed to read response body: {error}"))
    })?;

    Ok(ProxyResponse {
        status,
        headers,
        body,
    })
}

fn header_value_to_string(value: &ureq::http::HeaderValue) -> String {
    String::from_utf8_lossy(value.as_bytes()).into_owned()
}

fn connection_header_tokens(headers: &[(String, String)]) -> Vec<String> {
    let mut tokens = Vec::new();
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("connection") {
            push_connection_tokens(&mut tokens, value);
        }
    }
    tokens
}

fn header_map_connection_tokens(headers: &ureq::http::HeaderMap) -> Vec<String> {
    let mut tokens = Vec::new();
    for (name, value) in headers.iter() {
        if name.as_str().eq_ignore_ascii_case("connection") {
            let value = header_value_to_string(value);
            push_connection_tokens(&mut tokens, &value);
        }
    }
    tokens
}

fn push_connection_tokens(tokens: &mut Vec<String>, value: &str) {
    tokens.extend(
        value
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(|token| token.to_ascii_lowercase()),
    );
}

fn is_hop_by_hop_header(name: &str, connection_tokens: &[String]) -> bool {
    HOP_BY_HOP_HEADERS
        .iter()
        .any(|header| name.eq_ignore_ascii_case(header))
        || connection_tokens
            .iter()
            .any(|token| name.eq_ignore_ascii_case(token))
}

fn classify_ureq_error(error: ureq::Error) -> ProxyError {
    match error {
        ureq::Error::BadUri(message) => ProxyError::InvalidTarget(message),
        ureq::Error::Http(error) => ProxyError::InvalidRequest(error.to_string()),
        ureq::Error::Timeout(_) => ProxyError::Timeout,
        ureq::Error::HostNotFound => ProxyError::ConnectionFailed("host not found".to_string()),
        ureq::Error::ConnectionFailed => {
            ProxyError::ConnectionFailed("upstream connection failed".to_string())
        }
        ureq::Error::ConnectProxyFailed(message) => ProxyError::ConnectionFailed(message),
        ureq::Error::Io(error) => classify_io_error(error),
        other => ProxyError::UpstreamError(other.to_string()),
    }
}

fn classify_io_error(error: io::Error) -> ProxyError {
    match error.kind() {
        io::ErrorKind::TimedOut => ProxyError::Timeout,
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::NotConnected
        | io::ErrorKind::AddrInUse
        | io::ErrorKind::AddrNotAvailable
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::UnexpectedEof => ProxyError::ConnectionFailed(error.to_string()),
        _ => ProxyError::UpstreamError(format!("io error: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;

    #[derive(Debug)]
    struct CapturedRequest {
        request_line: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    fn spawn_one_shot_server(
        response_headers: &[(&str, &str)],
        response_body: &[u8],
    ) -> (
        String,
        mpsc::Receiver<CapturedRequest>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (sender, receiver) = mpsc::channel();
        let response = build_raw_response(response_headers, response_body);

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let captured = read_captured_request(&mut stream);
            sender.send(captured).unwrap();
            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
        });

        (address, receiver, handle)
    }

    fn build_raw_response(headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
        let mut response = b"HTTP/1.1 200 OK\r\n".to_vec();
        for (name, value) in headers {
            response.extend_from_slice(name.as_bytes());
            response.extend_from_slice(b": ");
            response.extend_from_slice(value.as_bytes());
            response.extend_from_slice(b"\r\n");
        }
        response.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        response.extend_from_slice(body);
        response
    }

    fn read_captured_request(stream: &mut TcpStream) -> CapturedRequest {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        let header_end = loop {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0, "request ended before headers were complete");
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(header_end) = crate::find_header_end(&buffer) {
                break header_end;
            }
        };

        let header_text = std::str::from_utf8(&buffer[..header_end]).unwrap();
        let mut lines = header_text.split("\r\n");
        let request_line = lines.next().unwrap().to_string();
        let headers = lines
            .filter_map(|line| {
                line.split_once(':')
                    .map(|(name, value)| (name.to_string(), value.trim().to_string()))
            })
            .collect::<Vec<_>>();

        let content_length = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = buffer[(header_end + 4)..].to_vec();
        while body.len() < content_length {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0, "request ended before body was complete");
            body.extend_from_slice(&chunk[..read]);
        }
        body.truncate(content_length);

        CapturedRequest {
            request_line,
            headers,
            body,
        }
    }

    fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn build_proxy_url_simple_path() {
        let config = ProxyConfig {
            target_url: "http://localhost:9222".into(),
            ..Default::default()
        };
        let url = build_proxy_url(&config, "/json/version").unwrap();
        assert_eq!(url, "http://localhost:9222/json/version");
    }

    #[test]
    fn build_proxy_url_with_trailing_slash() {
        let config = ProxyConfig {
            target_url: "http://localhost:9222/".into(),
            ..Default::default()
        };
        let url = build_proxy_url(&config, "/tabs").unwrap();
        assert_eq!(url, "http://localhost:9222/tabs");
    }

    #[test]
    fn build_proxy_url_invalid_target() {
        let config = ProxyConfig {
            target_url: "not a url".into(),
            ..Default::default()
        };
        let result = build_proxy_url(&config, "/test");
        assert!(matches!(result, Err(ProxyError::InvalidTarget(_))));
    }

    #[test]
    fn build_proxy_url_empty_path() {
        let config = ProxyConfig {
            target_url: "http://localhost:9222".into(),
            ..Default::default()
        };
        let url = build_proxy_url(&config, "").unwrap();
        assert_eq!(url, "http://localhost:9222/");
    }

    #[test]
    fn websocket_upgrade_detected() {
        let headers = vec![
            ("Connection".into(), "Upgrade".into()),
            ("Upgrade".into(), "websocket".into()),
        ];
        assert!(is_websocket_upgrade(&headers));
    }

    #[test]
    fn websocket_upgrade_case_insensitive() {
        let headers = vec![
            ("connection".into(), "upgrade".into()),
            ("upgrade".into(), "WebSocket".into()),
        ];
        assert!(is_websocket_upgrade(&headers));
    }

    #[test]
    fn websocket_upgrade_with_multi_value_connection() {
        let headers = vec![
            ("Connection".into(), "keep-alive, Upgrade".into()),
            ("Upgrade".into(), "websocket".into()),
        ];
        assert!(is_websocket_upgrade(&headers));
    }

    #[test]
    fn not_websocket_without_upgrade_header() {
        let headers = vec![("Connection".into(), "Upgrade".into())];
        assert!(!is_websocket_upgrade(&headers));
    }

    #[test]
    fn not_websocket_without_connection_upgrade() {
        let headers = vec![
            ("Connection".into(), "keep-alive".into()),
            ("Upgrade".into(), "websocket".into()),
        ];
        assert!(!is_websocket_upgrade(&headers));
    }

    #[test]
    fn not_websocket_empty_headers() {
        let headers: Vec<(String, String)> = vec![];
        assert!(!is_websocket_upgrade(&headers));
    }

    #[test]
    fn forward_request_invalid_target() {
        let config = ProxyConfig {
            target_url: "not valid".into(),
            ..Default::default()
        };
        let request = ProxyRequest {
            method: "GET".into(),
            path: "/test".into(),
            headers: vec![],
            body: None,
        };
        let result = forward_request(&config, &request);
        assert!(matches!(result, Err(ProxyError::InvalidTarget(_))));
    }

    #[test]
    fn forward_request_invalid_method() {
        let config = ProxyConfig {
            target_url: "http://127.0.0.1:9222".into(),
            ..Default::default()
        };
        let request = ProxyRequest {
            method: "BAD METHOD".into(),
            path: "/test".into(),
            headers: vec![],
            body: None,
        };
        let result = forward_request(&config, &request);
        assert!(matches!(result, Err(ProxyError::InvalidRequest(_))));
    }

    #[test]
    fn forward_request_connection_refused() {
        let config = ProxyConfig {
            target_url: "http://127.0.0.1:1".into(),
            timeout_ms: 2_000,
            preserve_host: false,
        };
        let request = ProxyRequest {
            method: "GET".into(),
            path: "/nope".into(),
            headers: vec![],
            body: None,
        };
        let result = forward_request(&config, &request);
        assert!(matches!(result, Err(ProxyError::ConnectionFailed(_))));
    }

    #[test]
    fn forward_request_filters_request_hop_by_hop_headers_and_response_hop_by_hop_headers() {
        let body = br#"{"ok":false}"#;
        let (target_url, receiver, handle) = spawn_one_shot_server(
            &[
                ("Content-Type", "application/json"),
                ("Location", "/instances/redirected"),
                ("Connection", "close, X-Response-Remove"),
                ("Keep-Alive", "timeout=5"),
                ("X-Response-Remove", "remove-me"),
            ],
            body,
        );
        let config = ProxyConfig {
            target_url,
            ..Default::default()
        };
        let request = ProxyRequest {
            method: "POST".into(),
            path: "/json/version?pretty=1".into(),
            headers: vec![
                ("Host".into(), "example.test".into()),
                ("Connection".into(), "keep-alive, X-Strip-Me".into()),
                ("Upgrade".into(), "websocket".into()),
                ("Te".into(), "trailers".into()),
                ("X-Strip-Me".into(), "remove-me".into()),
                ("X-Trace".into(), "trace-123".into()),
            ],
            body: Some(b"ping".to_vec()),
        };

        let response = forward_request(&config, &request).unwrap();
        let captured = receiver.recv().unwrap();
        handle.join().unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, body);
        assert_eq!(
            header_value(&response.headers, "content-type"),
            Some("application/json")
        );
        assert_eq!(
            header_value(&response.headers, "location"),
            Some("/instances/redirected")
        );
        assert_eq!(header_value(&response.headers, "connection"), None);
        assert_eq!(header_value(&response.headers, "keep-alive"), None);
        assert_eq!(header_value(&response.headers, "x-response-remove"), None);

        assert_eq!(
            captured.request_line,
            "POST /json/version?pretty=1 HTTP/1.1"
        );
        assert_eq!(captured.body, b"ping");
        assert_eq!(header_value(&captured.headers, "connection"), None);
        assert_eq!(header_value(&captured.headers, "upgrade"), None);
        assert_eq!(header_value(&captured.headers, "te"), None);
        assert_eq!(header_value(&captured.headers, "x-strip-me"), None);
        assert_eq!(
            header_value(&captured.headers, "x-trace"),
            Some("trace-123")
        );
        assert_ne!(
            header_value(&captured.headers, "host"),
            Some("example.test")
        );
    }

    #[test]
    fn forward_request_preserves_host_when_requested() {
        let (target_url, receiver, handle) =
            spawn_one_shot_server(&[("Content-Type", "text/plain")], b"ok");
        let config = ProxyConfig {
            target_url,
            preserve_host: true,
            ..Default::default()
        };
        let request = ProxyRequest {
            method: "GET".into(),
            path: "/json/list".into(),
            headers: vec![("Host".into(), "instance.example.test".into())],
            body: None,
        };

        let response = forward_request(&config, &request).unwrap();
        let captured = receiver.recv().unwrap();
        handle.join().unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"ok");
        assert_eq!(
            header_value(&captured.headers, "host"),
            Some("instance.example.test")
        );
    }

    #[test]
    fn proxy_error_display() {
        assert_eq!(
            ProxyError::InvalidTarget("bad".into()).to_string(),
            "invalid target: bad"
        );
        assert_eq!(
            ProxyError::InvalidRequest("bad method".into()).to_string(),
            "invalid request: bad method"
        );
        assert_eq!(
            ProxyError::ConnectionFailed("refused".into()).to_string(),
            "connection failed: refused"
        );
        assert_eq!(
            ProxyError::Timeout.to_string(),
            "upstream request timed out"
        );
        assert_eq!(
            ProxyError::UpstreamError("500".into()).to_string(),
            "upstream error: 500"
        );
    }
}
