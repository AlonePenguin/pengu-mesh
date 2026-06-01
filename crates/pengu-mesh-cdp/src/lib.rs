mod animations;

use crate::animations::{
    apply_animation_suppression, clear_animation_suppression, clear_reduced_motion_media_override,
    inject_animation_suppression, reduced_motion_media_override,
    remove_injected_animation_suppression,
};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use pengu_mesh_shared::{BrowserChannel, BrowserInstall};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tungstenite::{Message, connect};
use url::Url;

#[cfg(target_os = "macos")]
const REMOTE_DEBUGGING_APPROVAL_SCRIPT: &str = r#"
import subprocess
import sys
from AppKit import NSWorkspace
from ApplicationServices import (
    AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication,
    AXUIElementPerformAction,
    kAXChildrenAttribute,
    kAXDescriptionAttribute,
    kAXPressAction,
    kAXRoleAttribute,
    kAXTitleAttribute,
    kAXWindowsAttribute,
)

TARGET_SHEET = "Allow remote debugging?"
TARGET_BUTTON = "Allow"


def attr(element, key):
    err, value = AXUIElementCopyAttributeValue(element, key, None)
    return value if err == 0 else None


def children(element):
    value = attr(element, kAXChildrenAttribute)
    return list(value) if value else []


def walk(element):
    queue = [element]
    while queue:
        current = queue.pop(0)
        yield current
        queue.extend(children(current))


def button_matches(element):
    for key in (kAXTitleAttribute, kAXDescriptionAttribute):
        value = attr(element, key)
        if isinstance(value, str) and value.strip() == TARGET_BUTTON:
            return True
    return False


def running_pid(app_name):
    for app in NSWorkspace.sharedWorkspace().runningApplications():
        if app.localizedName() == app_name:
            return app.processIdentifier()
    return None


for app_name in sys.argv[1:]:
    pid = running_pid(app_name)
    if pid is None:
        continue
    subprocess.run(
        ["osascript", "-e", f'tell application "{app_name}" to activate'],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    app = AXUIElementCreateApplication(pid)
    windows = attr(app, kAXWindowsAttribute) or []
    for window in windows:
        for element in walk(window):
            if attr(element, kAXRoleAttribute) != "AXSheet":
                continue
            title = attr(element, kAXTitleAttribute)
            if title != TARGET_SHEET:
                continue
            for child in walk(element):
                if attr(child, kAXRoleAttribute) != "AXButton":
                    continue
                if not button_matches(child):
                    continue
                err = AXUIElementPerformAction(child, kAXPressAction)
                if err == 0:
                    print(f"clicked:{app_name}", end="")
                    sys.exit(0)
    continue

sys.exit(3)
"#;

#[cfg(target_os = "macos")]
const NATIVE_DIALOG_AUTOMATION_STATUS_SCRIPT: &str = r#"
from ApplicationServices import AXIsProcessTrusted
print("trusted" if AXIsProcessTrusted() else "untrusted", end="")
"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachMode {
    ManagedLaunch,
    ExternalAttach,
    Reconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMetadata {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugTarget {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "type")]
    pub target_type: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedLaunch {
    pub pid: u32,
    pub debug_http_url: String,
    pub browser_ws_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NavigationEvidence {
    pub final_url: String,
    pub load_event_fired: bool,
    pub duration_ms: u64,
}

pub fn discover_installations() -> Vec<BrowserInstall> {
    #[cfg(target_os = "macos")]
    let candidates = [
        (
            BrowserChannel::ChromeDev,
            "/Applications/Google Chrome Dev.app",
            "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev",
        ),
        (
            BrowserChannel::Chrome,
            "/Applications/Google Chrome.app",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ),
        (
            BrowserChannel::Chromium,
            "/Applications/Chromium.app",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ),
    ];

    #[cfg(not(target_os = "macos"))]
    let candidates = [(
        BrowserChannel::Chrome,
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome",
    )];

    candidates
        .into_iter()
        .map(|(channel, app_path, binary_path)| BrowserInstall {
            channel,
            app_path: app_path.to_string(),
            binary_path: binary_path.to_string(),
            installed: std::path::Path::new(app_path).exists(),
        })
        .collect()
}

pub fn find_installation(channel: BrowserChannel) -> Option<BrowserInstall> {
    discover_installations()
        .into_iter()
        .find(|candidate| candidate.channel == channel && candidate.installed)
}

pub fn reserve_debug_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind local debug port")?;
    let port = listener.local_addr().context("read local addr")?.port();
    drop(listener);
    Ok(port)
}

pub fn wait_for_debug_endpoint(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<VersionMetadata> {
    wait_for_debug_endpoint_for_apps(host, port, timeout, default_prompt_apps())
}

pub fn wait_for_debug_endpoint_for_apps(
    host: &str,
    port: u16,
    timeout: Duration,
    app_names: &[&str],
) -> Result<VersionMetadata> {
    let deadline = Instant::now() + timeout;
    let endpoint = format!("http://{host}:{port}/json/version");
    let mut last_error = String::from("endpoint not yet ready");
    let mut prompt_attempts = 0usize;
    let mut prompt_clicks = 0usize;
    let mut prompt_error: Option<String> = None;
    let mut failures = 0usize;
    while Instant::now() < deadline {
        match ureq::get(&endpoint).call() {
            Ok(response) => {
                let text = response
                    .into_body()
                    .read_to_string()
                    .with_context(|| format!("read {endpoint}"))?;
                let metadata: VersionMetadata =
                    serde_json::from_str(&text).with_context(|| format!("parse {endpoint}"))?;
                return Ok(metadata);
            }
            Err(error) => {
                last_error = error.to_string();
                failures += 1;
                if failures.is_multiple_of(3) && is_loopback_host(host) && !app_names.is_empty() {
                    prompt_attempts += 1;
                    match try_accept_remote_debugging_prompt(app_names) {
                        Ok(true) => {
                            prompt_clicks += 1;
                            std::thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            prompt_error = Some(error.to_string());
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        }
    }
    let mut suffix = String::new();
    if prompt_attempts > 0 {
        suffix.push_str(&format!(
            "; native prompt attempts={prompt_attempts}, clicks={prompt_clicks}"
        ));
    }
    if let Some(error) = prompt_error {
        suffix.push_str(&format!("; native prompt automation error={error}"));
    }
    bail!("timed out waiting for {endpoint}: {last_error}{suffix}")
}

pub fn launch_managed_browser(
    binary_path: &str,
    user_data_dir: &Path,
    port: u16,
    headless: bool,
) -> Result<ManagedLaunch> {
    if !Path::new(binary_path).exists() {
        bail!("browser binary does not exist: {binary_path}");
    }
    std::fs::create_dir_all(user_data_dir)
        .with_context(|| format!("create user data dir {}", user_data_dir.display()))?;
    let mut command = Command::new(binary_path);
    for arg in managed_browser_args(user_data_dir, port, headless) {
        command.arg(arg);
    }
    let child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("launch managed browser {binary_path}"))?;
    let version = wait_for_debug_endpoint_for_apps(
        "127.0.0.1",
        port,
        Duration::from_secs(15),
        prompt_apps_for_binary_path(binary_path),
    )?;
    Ok(ManagedLaunch {
        pid: child.id(),
        debug_http_url: debug_http_url("127.0.0.1", port),
        browser_ws_url: version.websocket_debugger_url,
    })
}

fn managed_browser_args(user_data_dir: &Path, port: u16, headless: bool) -> Vec<String> {
    let mut args = vec![
        format!("--remote-debugging-port={port}"),
        format!("--user-data-dir={}", user_data_dir.display()),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-sync".to_string(),
    ];
    if headless {
        args.push("--headless=new".to_string());
    }
    args.push("about:blank".to_string());
    args
}

pub fn native_dialog_automation_status() -> String {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("python3")
            .arg("-c")
            .arg(NATIVE_DIALOG_AUTOMATION_STATUS_SCRIPT)
            .output();
        match output {
            Ok(output) if output.status.success() => {
                match String::from_utf8_lossy(&output.stdout).trim() {
                    "trusted" => "trusted".into(),
                    "untrusted" => "untrusted".into(),
                    other if !other.is_empty() => format!("unknown ({other})"),
                    _ => "unknown".into(),
                }
            }
            Ok(output) => {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if detail.is_empty() {
                    "unavailable (python3 helper failed)".into()
                } else {
                    format!("unavailable ({detail})")
                }
            }
            Err(error) => format!("unavailable ({error})"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        "unsupported".into()
    }
}

fn default_prompt_apps() -> &'static [&'static str] {
    &["Google Chrome Dev", "Google Chrome", "Chromium"]
}

fn prompt_apps_for_binary_path(binary_path: &str) -> &'static [&'static str] {
    let normalized = binary_path.to_ascii_lowercase();
    if normalized.contains("google chrome dev") {
        &["Google Chrome Dev"]
    } else if normalized.contains("chromium") {
        &["Chromium"]
    } else {
        &["Google Chrome"]
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

fn try_accept_remote_debugging_prompt(app_names: &[&str]) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("python3")
            .arg("-c")
            .arg(REMOTE_DEBUGGING_APPROVAL_SCRIPT)
            .args(app_names)
            .output()
            .context("run native remote debugging approval helper")?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).contains("clicked:"));
        }
        if output.status.code() == Some(3) {
            return Ok(false);
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit status {}", output.status)
        };
        bail!("{detail}")
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_names;
        Ok(false)
    }
}

pub fn debug_http_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}")
}

pub fn list_targets(host: &str, port: u16) -> Result<Vec<DebugTarget>> {
    let endpoint = format!("http://{host}:{port}/json/list");
    let response = ureq::get(&endpoint)
        .call()
        .with_context(|| format!("GET {endpoint}"))?;
    let text = response
        .into_body()
        .read_to_string()
        .with_context(|| format!("read {endpoint}"))?;
    let targets: Vec<DebugTarget> =
        serde_json::from_str(&text).with_context(|| format!("parse {endpoint}"))?;
    Ok(targets
        .into_iter()
        .filter(|target| target.target_type == "page")
        .collect())
}

pub fn open_tab(host: &str, port: u16, url: &str) -> Result<DebugTarget> {
    let encoded_url: String = Url::parse("http://placeholder.invalid")
        .ok()
        .map(|_| url::form_urlencoded::byte_serialize(url.as_bytes()).collect())
        .unwrap_or_else(|| url.to_string());
    let endpoint = format!("http://{host}:{port}/json/new?{encoded_url}");
    let response = ureq::get(&endpoint)
        .call()
        .or_else(|_| ureq::put(&endpoint).send_empty())
        .with_context(|| format!("open tab via {endpoint}"))?;
    let text = response
        .into_body()
        .read_to_string()
        .with_context(|| format!("read {endpoint}"))?;
    let target: DebugTarget =
        serde_json::from_str(&text).with_context(|| format!("parse {endpoint}"))?;
    Ok(target)
}

pub fn close_tab(host: &str, port: u16, target_id: &str) -> Result<()> {
    let endpoint = format!("http://{host}:{port}/json/close/{target_id}");
    ureq::get(&endpoint)
        .call()
        .or_else(|_| ureq::put(&endpoint).send_empty())
        .with_context(|| format!("close tab via {endpoint}"))?;
    Ok(())
}

pub fn activate_tab(host: &str, port: u16, target_id: &str) -> Result<()> {
    let endpoint = format!("http://{host}:{port}/json/activate/{target_id}");
    ureq::get(&endpoint)
        .call()
        .or_else(|_| ureq::put(&endpoint).send_empty())
        .with_context(|| format!("activate tab via {endpoint}"))?;
    Ok(())
}

#[derive(Debug)]
pub struct CdpSession {
    socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    next_id: u64,
}

#[derive(Debug)]
struct AnimationSuppressionScope {
    script_identifier: String,
}

impl CdpSession {
    pub fn connect(websocket_url: &str) -> Result<Self> {
        let request_url = Url::parse(websocket_url).context("parse websocket URL")?;
        let (socket, _) = connect(request_url.as_str()).context("connect CDP websocket")?;
        Ok(Self { socket, next_id: 1 })
    }

    pub fn evaluate_json(&mut self, expression: &str) -> Result<Value> {
        let response = self.call(
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true,
            }),
        )?;
        if let Some(description) = runtime_evaluate_error(&response) {
            bail!("{description}");
        }
        if response["result"]["result"]["value"].is_null() {
            return Err(anyhow!("missing Runtime.evaluate result value"));
        }
        Ok(response["result"]["result"]["value"].clone())
    }

    pub fn evaluate_string(&mut self, expression: &str) -> Result<String> {
        let response = self.call(
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true,
            }),
        )?;
        if let Some(description) = runtime_evaluate_error(&response) {
            bail!("{description}");
        }
        response["result"]["result"]["value"]
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("missing Runtime.evaluate string value"))
    }

    pub fn navigate(&mut self, url: &str, timeout: Duration) -> Result<NavigationEvidence> {
        self.with_animation_suppression("navigation", |session| {
            let started = Instant::now();
            let deadline = started + timeout;
            let _ = session.call("Page.enable", json!({}))?;
            let response = session.call("Page.navigate", json!({ "url": url }))?;
            if let Some(error_text) = response["result"]["errorText"]
                .as_str()
                .filter(|value| !value.is_empty())
            {
                bail!("CDP Page.navigate failed: {error_text}");
            }
            let mut last_ready_state = "loading".to_string();
            loop {
                if let Ok(ready_state) = session.evaluate_string("document.readyState") {
                    last_ready_state = ready_state.clone();
                    if ready_state == "complete" {
                        if Instant::now() > deadline {
                            let final_url =
                                session.current_url().unwrap_or_else(|_| url.to_string());
                            bail!(
                                "navigation timed out waiting for load event; final_url={final_url}; last_ready_state={last_ready_state}"
                            );
                        }
                        return Ok(NavigationEvidence {
                            final_url: session.current_url().unwrap_or_else(|_| url.to_string()),
                            load_event_fired: true,
                            duration_ms: started.elapsed().as_millis() as u64,
                        });
                    }
                }
                if Instant::now() >= deadline {
                    let final_url = session.current_url().unwrap_or_else(|_| url.to_string());
                    bail!(
                        "navigation timed out waiting for load event; final_url={final_url}; last_ready_state={last_ready_state}"
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        let _ = self.call("Input.insertText", json!({ "text": text }))?;
        Ok(())
    }

    pub fn dispatch_key(&mut self, key: &str) -> Result<()> {
        for event_type in ["keyDown", "keyUp"] {
            let _ = self.call(
                "Input.dispatchKeyEvent",
                json!({
                    "type": event_type,
                    "key": key,
                    "text": if event_type == "keyDown" { key } else { "" },
                    "unmodifiedText": if event_type == "keyDown" { key } else { "" },
                }),
            )?;
        }
        Ok(())
    }

    pub fn capture_screenshot(&mut self, full_page: bool) -> Result<String> {
        self.with_animation_suppression("screenshot", |session| {
            let response = if full_page {
                let metrics = session.call("Page.getLayoutMetrics", json!({}))?;
                let content_size = &metrics["result"]["contentSize"];
                let width = content_size["width"]
                    .as_f64()
                    .ok_or_else(|| anyhow!("missing layout metrics width"))?;
                let height = content_size["height"]
                    .as_f64()
                    .ok_or_else(|| anyhow!("missing layout metrics height"))?;
                session.call(
                    "Page.captureScreenshot",
                    json!({
                        "format": "png",
                        "captureBeyondViewport": true,
                        "clip": {
                            "x": 0.0,
                            "y": 0.0,
                            "width": width.max(1.0),
                            "height": height.max(1.0),
                            "scale": 1.0
                        }
                    }),
                )?
            } else {
                session.call("Page.captureScreenshot", json!({"format": "png"}))?
            };
            response["result"]["data"]
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| anyhow!("missing screenshot data"))
        })
    }

    pub fn print_to_pdf(&mut self) -> Result<String> {
        self.with_animation_suppression("PDF print", |session| {
            let response = session.call(
                "Page.printToPDF",
                json!({
                    "printBackground": true,
                    "paperWidth": 8.27,
                    "paperHeight": 11.69,
                }),
            )?;
            response["result"]["data"]
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| anyhow!("missing PDF data"))
        })
    }

    pub fn start_tracing(&mut self, categories: &[&str]) -> Result<()> {
        let categories = if categories.is_empty() {
            vec![
                "devtools.timeline",
                "disabled-by-default-devtools.screenshot",
                "toplevel",
            ]
        } else {
            categories.to_vec()
        };
        let _ = self.call(
            "Tracing.start",
            json!({
                "transferMode": "ReturnAsStream",
                "categories": categories.join(","),
            }),
        )?;
        Ok(())
    }

    pub fn end_tracing_and_collect(&mut self) -> Result<Vec<u8>> {
        let request_id = self.send_request("Tracing.end", json!({}))?;
        let stream_handle = loop {
            let message = self.socket.read().context("read CDP trace message")?;
            match message {
                Message::Text(payload) => {
                    let value: Value = serde_json::from_str(&payload).context("parse CDP JSON")?;
                    if value["id"].as_u64() == Some(request_id) {
                        if !value["error"].is_null() {
                            bail!("CDP Tracing.end failed: {}", value["error"]);
                        }
                        continue;
                    }
                    if value["method"].as_str() == Some("Tracing.tracingComplete") {
                        break value["params"]["stream"].as_str().map(ToOwned::to_owned);
                    }
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Binary(_) => continue,
                Message::Close(frame) => {
                    bail!("CDP websocket closed while waiting for tracingComplete: {frame:?}")
                }
            }
        }
        .ok_or_else(|| anyhow!("missing trace stream handle"))?;
        let mut bytes = Vec::new();
        loop {
            let response = self.call("IO.read", json!({"handle": stream_handle, "size": 65536}))?;
            let data = response["result"]["data"].as_str().unwrap_or_default();
            if response["result"]["base64Encoded"]
                .as_bool()
                .unwrap_or(false)
            {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .context("decode base64 trace chunk")?;
                bytes.extend_from_slice(&decoded);
            } else {
                bytes.extend_from_slice(data.as_bytes());
            }
            if response["result"]["eof"].as_bool().unwrap_or(false) {
                break;
            }
        }
        let _ = self.call("IO.close", json!({"handle": stream_handle}));
        Ok(bytes)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.send_request(method, params)?;
        loop {
            let message = self.socket.read().context("read CDP response")?;
            match message {
                Message::Text(payload) => {
                    let value: Value = serde_json::from_str(&payload).context("parse CDP JSON")?;
                    if value["id"].as_u64() != Some(id) {
                        continue;
                    }
                    if !value["error"].is_null() {
                        bail!("CDP {method} failed: {}", value["error"]);
                    }
                    return Ok(value);
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Binary(_) => continue,
                Message::Close(frame) => {
                    bail!("CDP websocket closed while waiting for {method}: {frame:?}")
                }
            }
        }
    }

    fn call_message(&mut self, message: Value) -> Result<Value> {
        let method = message["method"]
            .as_str()
            .ok_or_else(|| anyhow!("CDP message missing method"))?;
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        self.call(method, params)
    }

    fn with_animation_suppression<T>(
        &mut self,
        action: &str,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let suppression = self
            .begin_animation_suppression()
            .with_context(|| format!("enable animation suppression for {action}"))?;
        let result = operation(self);
        let cleanup = self
            .end_animation_suppression(suppression)
            .with_context(|| format!("cleanup animation suppression after {action}"));
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(action_error), Ok(())) => Err(action_error),
            (Ok(_), Err(cleanup_error)) => Err(cleanup_error),
            (Err(action_error), Err(cleanup_error)) => Err(anyhow!(
                "{action} failed: {action_error}; cleanup also failed: {cleanup_error}"
            )),
        }
    }

    fn begin_animation_suppression(&mut self) -> Result<AnimationSuppressionScope> {
        let _ = self
            .call_message(reduced_motion_media_override())
            .context("set prefers-reduced-motion override")?;
        let registration = self
            .call_message(inject_animation_suppression())
            .map_err(|error| {
                combine_operation_and_cleanup_error(
                    "register animation suppression for new documents",
                    error,
                    self.clear_animation_suppression_state(None),
                )
            })?;
        let script_identifier = registration["result"]["identifier"]
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("missing animation suppression script identifier"))
            .map_err(|error| {
                combine_operation_and_cleanup_error(
                    "capture animation suppression registration identifier",
                    error,
                    self.clear_animation_suppression_state(None),
                )
            })?;
        let apply_response = self
            .call_message(apply_animation_suppression())
            .map_err(|error| {
                combine_operation_and_cleanup_error(
                    "apply animation suppression to current document",
                    error,
                    self.clear_animation_suppression_state(Some(&script_identifier)),
                )
            })?;
        if let Some(description) = runtime_evaluate_error(&apply_response) {
            return Err(combine_operation_and_cleanup_error(
                "apply animation suppression to current document",
                anyhow!("{description}"),
                self.clear_animation_suppression_state(Some(&script_identifier)),
            ));
        }
        Ok(AnimationSuppressionScope { script_identifier })
    }

    fn end_animation_suppression(&mut self, scope: AnimationSuppressionScope) -> Result<()> {
        self.clear_animation_suppression_state(Some(&scope.script_identifier))
    }

    fn clear_animation_suppression_state(&mut self, script_identifier: Option<&str>) -> Result<()> {
        let mut errors = Vec::new();

        match self.call_message(clear_animation_suppression()) {
            Ok(response) => {
                if let Some(description) = runtime_evaluate_error(&response) {
                    errors.push(format!(
                        "remove animation suppression from current document: {description}"
                    ));
                }
            }
            Err(error) => errors.push(format!(
                "remove animation suppression from current document: {error}"
            )),
        }

        if let Some(identifier) = script_identifier
            && let Err(error) = self.call_message(remove_injected_animation_suppression(identifier))
        {
            errors.push(format!(
                "remove new-document animation suppression hook: {error}"
            ));
        }

        if let Err(error) = self.call_message(clear_reduced_motion_media_override()) {
            errors.push(format!("clear prefers-reduced-motion override: {error}"));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            bail!(errors.join("; "))
        }
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.socket
            .send(Message::Text(request.to_string().into()))
            .context("send CDP request")?;
        Ok(id)
    }

    fn current_url(&mut self) -> Result<String> {
        let response = self.call("Page.getNavigationHistory", json!({}))?;
        let current_index = response["result"]["currentIndex"]
            .as_u64()
            .ok_or_else(|| anyhow!("missing current navigation index"))?
            as usize;
        let entries = response["result"]["entries"]
            .as_array()
            .ok_or_else(|| anyhow!("missing navigation history entries"))?;
        entries
            .get(current_index)
            .and_then(|entry| entry["url"].as_str())
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("missing navigation history url"))
    }
}

fn runtime_evaluate_error(response: &Value) -> Option<String> {
    let details = response["result"]["exceptionDetails"].as_object()?;
    Some(
        details
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| {
                response["result"]["result"]["description"]
                    .as_str()
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or("Runtime.evaluate failed")
            .to_string(),
    )
}

fn combine_operation_and_cleanup_error(
    action: &str,
    operation_error: anyhow::Error,
    cleanup_result: Result<()>,
) -> anyhow::Error {
    match cleanup_result {
        Ok(()) => operation_error.context(action.to_string()),
        Err(cleanup_error) => {
            anyhow!("{action}: {operation_error}; cleanup also failed: {cleanup_error}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_prompt_apps, discover_installations, is_loopback_host, managed_browser_args,
        prompt_apps_for_binary_path,
    };
    use pengu_mesh_shared::BrowserChannel;
    use std::path::Path;

    #[test]
    fn prefers_chrome_dev_on_macos() {
        let installs = discover_installations();
        assert!(
            installs
                .iter()
                .any(|item| item.channel == BrowserChannel::ChromeDev)
        );
        assert!(
            installs
                .iter()
                .any(|item| item.installed || item.binary_path.contains("Chrom"))
        );
    }

    #[test]
    fn prompt_app_mapping_prefers_expected_browser() {
        assert_eq!(
            prompt_apps_for_binary_path(
                "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev"
            ),
            ["Google Chrome Dev"]
        );
        assert_eq!(
            prompt_apps_for_binary_path("/Applications/Chromium.app/Contents/MacOS/Chromium"),
            ["Chromium"]
        );
        assert_eq!(
            prompt_apps_for_binary_path(
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
            ),
            ["Google Chrome"]
        );
        assert_eq!(
            default_prompt_apps(),
            ["Google Chrome Dev", "Google Chrome", "Chromium"]
        );
    }

    #[test]
    fn loopback_hosts_are_recognized() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("[::1]"));
        assert!(!is_loopback_host("192.168.1.10"));
    }

    #[test]
    fn managed_browser_args_include_headless_flag_only_when_requested() {
        let headed = managed_browser_args(Path::new("/tmp/profile"), 9222, false);
        let headless = managed_browser_args(Path::new("/tmp/profile"), 9222, true);
        assert!(!headed.iter().any(|arg| arg == "--headless=new"));
        assert!(headless.iter().any(|arg| arg == "--headless=new"));
        assert_eq!(headless.last().map(String::as_str), Some("about:blank"));
    }
}
