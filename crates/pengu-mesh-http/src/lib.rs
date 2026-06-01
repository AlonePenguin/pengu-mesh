#[allow(dead_code)]
mod proxy;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug, Clone, Serialize)]
pub struct RouteSurface {
    pub route: &'static str,
    pub role: &'static str,
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body,
        }
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
}

pub fn bootstrap_routes() -> Vec<RouteSurface> {
    vec![
        RouteSurface {
            route: "/health",
            role: "runtime-and-readiness-summary",
        },
        RouteSurface {
            route: "/doctor",
            role: "environment-and-runtime-diagnostics",
        },
        RouteSurface {
            route: "/diagnose",
            role: "side-effect-free-host-remediation-diagnostics",
        },
        RouteSurface {
            route: "/capabilities/preflight",
            role: "capability-policy-preflight",
        },
        RouteSurface {
            route: "/host/access/status",
            role: "host-access-status",
        },
        RouteSurface {
            route: "/host/access/setup",
            role: "host-access-setup",
        },
        RouteSurface {
            route: "/profiles",
            role: "managed-profile-inventory",
        },
        RouteSurface {
            route: "/profiles/create",
            role: "managed-profile-create",
        },
        RouteSurface {
            route: "/instances",
            role: "browser-instance-lifecycle",
        },
        RouteSurface {
            route: "/instances/start",
            role: "managed-browser-launch",
        },
        RouteSurface {
            route: "/instances/attach",
            role: "external-browser-attach",
        },
        RouteSurface {
            route: "/instances/stop",
            role: "managed-browser-stop",
        },
        RouteSurface {
            route: "/leases",
            role: "active-lease-status",
        },
        RouteSurface {
            route: "/leases/acquire",
            role: "lease-acquire-or-renew",
        },
        RouteSurface {
            route: "/leases/release",
            role: "lease-release",
        },
        RouteSurface {
            route: "/leases/transfer",
            role: "lease-transfer",
        },
        RouteSurface {
            route: "/tabs",
            role: "tab-inventory",
        },
        RouteSurface {
            route: "/tabs/actions",
            role: "tab-action-catalog",
        },
        RouteSurface {
            route: "/browser/surfaces",
            role: "browser-surface-inventory",
        },
        RouteSurface {
            route: "/browser/surfaces/actions",
            role: "browser-surface-action-catalog",
        },
        RouteSurface {
            route: "/browser/surfaces/snapshot",
            role: "browser-surface-snapshot",
        },
        RouteSurface {
            route: "/browser/surfaces/action",
            role: "browser-surface-action",
        },
        RouteSurface {
            route: "/tabs/open",
            role: "tab-open",
        },
        RouteSurface {
            route: "/tabs/close",
            role: "tab-close",
        },
        RouteSurface {
            route: "/tabs/action",
            role: "typed-tab-action",
        },
        RouteSurface {
            route: "/tabs/snapshot",
            role: "tab-snapshot",
        },
        RouteSurface {
            route: "/tabs/text",
            role: "tab-text",
        },
        RouteSurface {
            route: "/tabs/screenshot",
            role: "tab-screenshot",
        },
        RouteSurface {
            route: "/tabs/pdf",
            role: "tab-pdf",
        },
        RouteSurface {
            route: "/artifacts",
            role: "artifact-list",
        },
        RouteSurface {
            route: "/artifacts/verify",
            role: "artifact-integrity-verification",
        },
        RouteSurface {
            route: "/artifacts/crop",
            role: "artifact-crop",
        },
        RouteSurface {
            route: "/artifacts/crop-grid",
            role: "artifact-crop-grid",
        },
        RouteSurface {
            route: "/artifacts/:id",
            role: "artifact-handle-resolution",
        },
        RouteSurface {
            route: "/capture/start",
            role: "capture-run-start",
        },
        RouteSurface {
            route: "/capture/stop",
            role: "capture-run-stop",
        },
        RouteSurface {
            route: "/runs",
            role: "capture-run-inventory",
        },
        RouteSurface {
            route: "/scenarios",
            role: "scenario-run-inventory",
        },
        RouteSurface {
            route: "/scenarios/summary",
            role: "scenario-evidence-summary",
        },
        RouteSurface {
            route: "/scenarios/gate",
            role: "scenario-evidence-gate",
        },
        RouteSurface {
            route: "/scenarios/:id",
            role: "scenario-run-detail",
        },
        RouteSurface {
            route: "/events",
            role: "capture-event-tail",
        },
        RouteSurface {
            route: "/replay/export",
            role: "replay-manifest-export",
        },
        RouteSurface {
            route: "/trace/capture",
            role: "trace-artifact-capture",
        },
        RouteSurface {
            route: "/recording/capture",
            role: "recording-artifact-capture",
        },
        RouteSurface {
            route: "/tools",
            role: "generic-tool-catalog",
        },
        RouteSurface {
            route: "/tools/:tool",
            role: "generic-tool-dispatch",
        },
    ]
}

pub fn read_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut chunk).context("read request bytes")?;
        if read == 0 {
            bail!("unexpected eof while reading request");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
        anyhow::ensure!(buffer.len() <= 1024 * 1024, "request headers too large");
    };
    let header_bytes = &buffer[..header_end];
    let mut body = buffer[(header_end + 4)..].to_vec();
    let header_text = std::str::from_utf8(header_bytes).context("request headers are not utf-8")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("missing request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing request method"))?
        .to_uppercase();
    let target = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing request target"))?;
    let mut content_length = 0_usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value
                .trim()
                .parse::<usize>()
                .context("parse content-length")?;
        }
    }
    while body.len() < content_length {
        let read = stream.read(&mut chunk).context("read request body")?;
        if read == 0 {
            bail!("unexpected eof while reading request body");
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);
    let (path, query) = parse_target(target);
    Ok(HttpRequest {
        method,
        path,
        query,
        body,
    })
}

pub fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> Result<()> {
    let status_text = status_text(response.status);
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        status_text,
        response.content_type,
        response.body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .context("write response headers")?;
    stream
        .write_all(&response.body)
        .context("write response body")?;
    stream.flush().context("flush response")?;
    Ok(())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_target(target: &str) -> (String, BTreeMap<String, String>) {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<BTreeMap<_, _>>();
    (path.to_string(), query)
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        409 => "Conflict",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

#[cfg(test)]
mod tests {
    use super::bootstrap_routes;

    #[test]
    fn bootstrap_routes_include_new_profile_and_tab_actions() {
        let routes = bootstrap_routes();
        assert!(
            routes
                .iter()
                .any(|route| route.route == "/capabilities/preflight")
        );
        assert!(routes.iter().any(|route| route.route == "/profiles/create"));
        assert!(routes.iter().any(|route| route.route == "/tabs/action"));
        assert!(routes.iter().any(|route| route.route == "/tabs/actions"));
        assert!(routes.iter().any(|route| route.route == "/artifacts"));
        assert!(
            routes
                .iter()
                .any(|route| route.route == "/artifacts/verify")
        );
        assert!(
            routes
                .iter()
                .any(|route| route.route == "/browser/surfaces/actions")
        );
    }
}
