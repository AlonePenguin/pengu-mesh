use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::process::Command;

use pengu_mesh_shared::{
    AssistiveOverlayDescriptor, BrowserInstance, BrowserSurfaceActionPayload,
    BrowserSurfaceActionRequest, BrowserSurfaceDescriptor, BrowserSurfaceListPayload,
    ExecutionChannel, ExecutionChannelAvailability, HostAccessProbe, HostAccessService,
    HostAccessSetupMode, HostAccessSetupRequest, HostAccessSetupResult, HostAccessSetupStep,
    HostAccessStatus, InterferenceLevel, PermissionState,
};

#[derive(Debug, Clone, Deserialize)]
struct AssistiveOverlayManifest {
    overlays: Vec<AssistiveOverlayDescriptor>,
}

#[derive(Debug, Clone, Deserialize)]
struct SettingsManifest {
    services: Vec<SettingsService>,
}

#[derive(Debug, Clone, Deserialize)]
struct SettingsService {
    service: HostAccessService,
    open_settings_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeStatusPayload {
    services: Vec<HostAccessProbe>,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeSurfacePayload {
    app_name: String,
    surfaces: Vec<BrowserSurfaceDescriptor>,
}

#[derive(Debug, Clone, Deserialize)]
struct NativeSurfaceSnapshot {
    app_name: String,
    surfaces: Vec<BrowserSurfaceDescriptor>,
    capture_path: Option<String>,
    capture_source: Option<String>,
    capture_detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeActionPayload {
    app_name: String,
    resolved_channel: ExecutionChannel,
    interference_level: InterferenceLevel,
    took_focus: bool,
    fallback_count: u8,
    #[serde(default)]
    target_focused: bool,
    detail: String,
}

#[derive(Debug, Clone)]
pub struct BrowserSurfaceSnapshotResult {
    pub app_name: String,
    pub surfaces: Vec<BrowserSurfaceDescriptor>,
    pub capture_path: Option<String>,
    pub capture_source: Option<String>,
    pub capture_detail: Option<String>,
}

const DEFAULT_SETUP_SERVICES: [HostAccessService; 7] = [
    HostAccessService::Accessibility,
    HostAccessService::ScreenCapture,
    HostAccessService::ListenEvent,
    HostAccessService::AppleEventsChrome,
    HostAccessService::AppleEventsChromeDev,
    HostAccessService::AppleEventsChromium,
    HostAccessService::DevtoolsSecurity,
];

const BRIDGE_SCRIPT: &str = r#"
import json
import os
import subprocess
import sys
import tempfile

from ApplicationServices import (
    AXIsProcessTrusted,
    AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication,
    AXUIElementPerformAction,
    AXUIElementPostKeyboardEvent,
    AXUIElementSetAttributeValue,
    kAXChildrenAttribute,
    kAXDescriptionAttribute,
    kAXEnabledAttribute,
    kAXFocusedAttribute,
    kAXMainAttribute,
    kAXPressAction,
    kAXRoleAttribute,
    kAXTitleAttribute,
    kAXValueAttribute,
    kAXWindowsAttribute,
)
from Quartz.CoreGraphics import (
    CGPreflightListenEventAccess,
    CGPreflightScreenCaptureAccess,
    CGRequestListenEventAccess,
    CGRequestScreenCaptureAccess,
    CGWindowListCopyWindowInfo,
    kCGNullWindowID,
    kCGWindowListOptionAll,
)

try:
    from ApplicationServices import AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt
except Exception:
    AXIsProcessTrustedWithOptions = None
    kAXTrustedCheckOptionPrompt = None


TARGET_ROLES = {
    "AXApplication",
    "AXWindow",
    "AXSheet",
    "AXDialog",
    "AXToolbar",
    "AXGroup",
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXMenuButton",
    "AXPopUpButton",
    "AXTextField",
    "AXTextArea",
    "AXStaticText",
    "AXTabGroup",
    "AXScrollArea",
}
PRESSABLE_ROLES = {
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXMenuButton",
    "AXPopUpButton",
}
TEXT_ROLES = {"AXTextField", "AXTextArea"}
SPECIAL_KEY_CODES = {
    "return": 36,
    "enter": 36,
    "tab": 48,
    "space": 49,
    "escape": 53,
    "esc": 53,
    "left": 123,
    "right": 124,
    "down": 125,
    "up": 126,
}


def state_for_bool(value):
    return "granted" if bool(value) else "missing"


def textish(value):
    if value is None:
        return None
    if isinstance(value, str):
        normalized = value.strip()
        return normalized or None
    if isinstance(value, (int, float, bool)):
        return str(value)
    return None


def attr(element, key):
    err, value = AXUIElementCopyAttributeValue(element, key, None)
    return value if err == 0 else None


def children(element):
    value = attr(element, kAXChildrenAttribute)
    return list(value) if value else []


def running_pid(app_name):
    from AppKit import NSWorkspace

    for app in NSWorkspace.sharedWorkspace().runningApplications():
        if app.localizedName() == app_name:
            return app.processIdentifier()
    return None


def app_name_for_channel(channel):
    channel = (channel or "").strip().lower()
    if channel == "chrome-dev":
        return "Google Chrome Dev"
    if channel == "chrome":
        return "Google Chrome"
    if channel == "chromium":
        return "Chromium"
    return "Google Chrome Dev"


def apple_events_service_for_app(app_name):
    if "Chrome Dev" in app_name:
        return "apple_events_chrome_dev"
    if app_name == "Chromium":
        return "apple_events_chromium"
    return "apple_events_chrome"


def apple_events_probe(app_name):
    script = '''
on run argv
  set appName to item 1 of argv
  tell application appName to get name
end run
'''
    result = subprocess.run(
        ["osascript", "-", app_name],
        input=script,
        text=True,
        capture_output=True,
        check=False,
    )
    stderr = (result.stderr or "").strip()
    if result.returncode == 0:
        return {
            "service": apple_events_service_for_app(app_name),
            "state": "granted",
            "requestable": True,
            "open_settings_url": None,
            "detail": "Apple Events probe succeeded",
        }
    if "not authorized" in stderr or "-1743" in stderr:
        state = "missing"
    elif "Can't get application" in stderr:
        state = "unsupported"
    else:
        state = "unknown"
    return {
        "service": apple_events_service_for_app(app_name),
        "state": state,
        "requestable": True,
        "open_settings_url": None,
        "detail": stderr or f"Apple Events probe failed with exit {result.returncode}",
    }


def read_only_apple_events_status(app_name):
    return {
        "service": apple_events_service_for_app(app_name),
        "state": "unknown",
        "requestable": True,
        "open_settings_url": None,
        "detail": "Read-only diagnostics do not probe Automation to avoid consent prompts",
    }


def prompt_accessibility():
    if AXIsProcessTrustedWithOptions is None or kAXTrustedCheckOptionPrompt is None:
        return bool(AXIsProcessTrusted())
    try:
        AXIsProcessTrustedWithOptions({kAXTrustedCheckOptionPrompt: True})
    except Exception:
        pass
    return bool(AXIsProcessTrusted())


def activate_app(app_name):
    subprocess.run(
        ["osascript", "-e", f'tell application "{app_name}" to activate'],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def supports_actions(role):
    actions = []
    if role in PRESSABLE_ROLES:
        actions.append("press")
        actions.append("confirm")
    if role in TEXT_ROLES:
        actions.append("set_value")
    if role in {"AXApplication", "AXWindow", "AXSheet", "AXDialog"}:
        actions.append("key_sequence")
        actions.append("focus")
        actions.append("confirm")
    if role not in TEXT_ROLES and role not in PRESSABLE_ROLES:
        actions.append("focus")
    seen = []
    for item in actions:
        if item not in seen:
            seen.append(item)
    return seen


def collect_surfaces(app_name, channel, pid, instance_id, max_nodes=600):
    if pid is None:
        pid = running_pid(app_name)
    if not AXIsProcessTrusted():
        raise SystemExit("requires accessibility permission")
    if pid is None:
        raise SystemExit(f"browser app not running: {app_name}")
    app = AXUIElementCreateApplication(pid)
    queue = [(app, "0", None, None)]
    surfaces = []
    path_map = {"0": app}
    visited = 0
    while queue and visited < max_nodes:
        element, path, parent_id, inherited_window = queue.pop(0)
        visited += 1
        role = textish(attr(element, kAXRoleAttribute)) or "AXUnknown"
        title = textish(attr(element, kAXTitleAttribute))
        description = textish(attr(element, kAXDescriptionAttribute))
        value = textish(attr(element, kAXValueAttribute))
        current_window = inherited_window
        if role in {"AXWindow", "AXSheet", "AXDialog"} and title:
            current_window = title
        if path == "0" or role in TARGET_ROLES:
            surfaces.append({
                "id": f"ax:{path}",
                "parent_id": parent_id,
                "path": path,
                "role": role,
                "title": title,
                "description": description,
                "value": value,
                "window_title": current_window,
                "actions": supports_actions(role),
                "focused": bool(attr(element, kAXFocusedAttribute) or False),
                "enabled": bool(attr(element, kAXEnabledAttribute) if attr(element, kAXEnabledAttribute) is not None else True),
                "app_name": app_name,
                "channel": channel,
                "instance_id": instance_id,
            })
        for index, child in enumerate(children(element)):
            child_path = f"{path}/{index}"
            path_map[child_path] = child
            queue.append((child, child_path, f"ax:{path}", current_window))
    return app, surfaces, path_map


def read_status(payload):
    chrome = read_only_apple_events_status("Google Chrome")
    chrome_dev = read_only_apple_events_status("Google Chrome Dev")
    chromium = read_only_apple_events_status("Chromium")
    services = [
        {
            "service": "accessibility",
            "state": state_for_bool(AXIsProcessTrusted()),
            "requestable": True,
            "open_settings_url": None,
            "detail": "AXIsProcessTrusted",
        },
        {
            "service": "screen_capture",
            "state": state_for_bool(CGPreflightScreenCaptureAccess()),
            "requestable": True,
            "open_settings_url": None,
            "detail": "CGPreflightScreenCaptureAccess",
        },
        {
            "service": "listen_event",
            "state": state_for_bool(CGPreflightListenEventAccess()),
            "requestable": True,
            "open_settings_url": None,
            "detail": "CGPreflightListenEventAccess",
        },
        chrome,
        chrome_dev,
        chromium,
    ]
    return {"services": services}


def request_service(payload):
    service = payload.get("service")
    app_name = payload.get("app_name")
    if service == "accessibility":
        granted = prompt_accessibility()
        return {"ok": granted, "state": state_for_bool(granted), "detail": "Accessibility prompt requested"}
    if service == "screen_capture":
        granted = bool(CGRequestScreenCaptureAccess())
        return {"ok": granted, "state": state_for_bool(granted), "detail": "Screen capture request issued"}
    if service == "listen_event":
        granted = bool(CGRequestListenEventAccess())
        return {"ok": granted, "state": state_for_bool(granted), "detail": "Listen-event request issued"}
    if service in {"apple_events_chrome", "apple_events_chrome_dev", "apple_events_chromium"}:
        if service == "apple_events_chromium":
            target = app_name or "Chromium"
        else:
            target = app_name or ("Google Chrome Dev" if service.endswith("dev") else "Google Chrome")
        probe = apple_events_probe(target)
        return {"ok": probe["state"] == "granted", "state": probe["state"], "detail": probe["detail"]}
    raise SystemExit(f"unsupported host access service: {service}")


def pick_capture_window(app_name, window_title):
    info = CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID) or []
    best = None
    for item in info:
        owner = textish(item.get("kCGWindowOwnerName"))
        if owner != app_name:
            continue
        number = item.get("kCGWindowNumber")
        if number is None:
            continue
        name = textish(item.get("kCGWindowName"))
        layer = int(item.get("kCGWindowLayer", 0))
        score = 0
        if layer == 0:
            score += 1
        if window_title and name == window_title:
            score += 4
        elif window_title and name and window_title in name:
            score += 3
        elif name:
            score += 2
        if best is None or score > best[0]:
            best = (score, int(number), name)
    return best


def capture_window(app_name, window_title):
    best = pick_capture_window(app_name, window_title)
    if best is None:
        return {"capture_path": None, "capture_source": None, "capture_detail": "no matching window for capture"}
    _, window_number, name = best
    handle, path = tempfile.mkstemp(prefix="pengu-mesh-surface-", suffix=".png")
    os.close(handle)
    result = subprocess.run(
        ["screencapture", "-x", "-l", str(window_number), path],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    if result.returncode == 0 and os.path.exists(path):
        return {
            "capture_path": path,
            "capture_source": "screencapture",
            "capture_detail": f"captured window {window_number} ({name or 'unnamed'})",
        }
    if os.path.exists(path):
        os.unlink(path)
    return {
        "capture_path": None,
        "capture_source": None,
        "capture_detail": (result.stderr or "").strip() or f"screencapture failed for window {window_number}",
    }


def find_surface(path_map, surface_id):
    if not surface_id:
        return path_map["0"], "0"
    path = surface_id.split("ax:", 1)[-1]
    element = path_map.get(path)
    if element is None:
        raise SystemExit(f"surface not found: {surface_id}")
    return element, path


def press_element(element):
    return AXUIElementPerformAction(element, kAXPressAction) == 0


def focus_element(element):
    try:
        if AXUIElementSetAttributeValue(element, kAXFocusedAttribute, True) == 0:
            return True
    except Exception:
        pass
    try:
        if AXUIElementSetAttributeValue(element, kAXMainAttribute, True) == 0:
            return True
    except Exception:
        pass
    return False


def surface_has_focus(element):
    focused = attr(element, kAXFocusedAttribute)
    if focused is not None:
        return bool(focused)
    main = attr(element, kAXMainAttribute)
    if main is not None:
        return bool(main)
    return False


def set_value(element, value):
    return AXUIElementSetAttributeValue(element, kAXValueAttribute, value) == 0


def app_scoped_key(app_element, key_name):
    if not key_name:
        return False
    key = key_name.strip().lower()
    if key in SPECIAL_KEY_CODES:
        code = SPECIAL_KEY_CODES[key]
        AXUIElementPostKeyboardEvent(app_element, 0, code, True)
        AXUIElementPostKeyboardEvent(app_element, 0, code, False)
        return True
    if len(key_name) == 1:
        ch = key_name
        AXUIElementPostKeyboardEvent(app_element, ch, 0, True)
        AXUIElementPostKeyboardEvent(app_element, ch, 0, False)
        return True
    return False


def global_key(app_name, key_name):
    key = (key_name or "").strip().lower()
    activate_app(app_name)
    if key in SPECIAL_KEY_CODES:
        key_code = SPECIAL_KEY_CODES[key]
        script = f'''
tell application "System Events"
  key code {key_code}
end tell
'''
    elif len(key_name or "") == 1:
        script = f'''
tell application "System Events"
  keystroke "{key_name}"
end tell
'''
    else:
        return False
    result = subprocess.run(
        ["osascript", "-e", script],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode == 0


def preferred_channels(requested_channel, action):
    if requested_channel:
        return [requested_channel]
    if action in {"press", "focus", "set_value"}:
        return ["ax_direct", "apple_events_activation"]
    if action == "confirm":
        return ["ax_direct", "apple_events_activation", "app_scoped_key_post", "global_takeover"]
    if action == "key_sequence":
        return ["app_scoped_key_post", "global_takeover"]
    return ["ax_direct"]


def surface_action(payload):
    app_name = payload["app_name"]
    channel = payload["channel"]
    pid = payload.get("pid")
    instance_id = payload["instance_id"]
    request = payload["request"]
    app_element, surfaces, path_map = collect_surfaces(app_name, channel, pid, instance_id)
    target, _ = find_surface(path_map, request.get("surface_id"))
    requested_channel = request.get("execution_channel")
    allow_takeover = True if request.get("allow_takeover") is None else bool(request.get("allow_takeover"))
    action = request.get("action")
    fallback_count = 0
    for channel in preferred_channels(requested_channel, action):
        if channel == "global_takeover" and not allow_takeover:
            continue
        if channel == "ax_direct":
            if action == "press" and press_element(target):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "background_safe", "took_focus": False, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": "pressed surface through AX"}
            if action == "focus" and focus_element(target) and surface_has_focus(target):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "background_safe", "took_focus": False, "fallback_count": fallback_count, "target_focused": True, "detail": "focused surface through AX"}
            if action == "confirm" and press_element(target):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "background_safe", "took_focus": False, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": "confirmed surface through AX"}
            if action == "set_value":
                value = request.get("value")
                if not value:
                    raise SystemExit("set_value requires value")
                if set_value(target, value):
                    return {"app_name": app_name, "resolved_channel": channel, "interference_level": "background_safe", "took_focus": False, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": "set surface value through AX"}
        elif channel == "apple_events_activation":
            activate_app(app_name)
            if action == "press" and press_element(target):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "app_takeover", "took_focus": True, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": "activated app then pressed surface through AX"}
            if action == "focus" and focus_element(target) and surface_has_focus(target):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "app_takeover", "took_focus": True, "fallback_count": fallback_count, "target_focused": True, "detail": "activated app then focused surface through AX"}
            if action == "confirm" and press_element(target):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "app_takeover", "took_focus": True, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": "activated app then confirmed surface through AX"}
            if action == "set_value":
                value = request.get("value")
                if not value:
                    raise SystemExit("set_value requires value")
                if set_value(target, value):
                    return {"app_name": app_name, "resolved_channel": channel, "interference_level": "app_takeover", "took_focus": True, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": "activated app then set value through AX"}
        elif channel == "app_scoped_key_post":
            key_name = request.get("key_sequence") or "return"
            if app_scoped_key(app_element, key_name):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "background_safe", "took_focus": False, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": f"posted {key_name} to app"}
        elif channel == "global_takeover":
            key_name = request.get("key_sequence") or "return"
            if action in {"confirm", "key_sequence"} and global_key(app_name, key_name):
                return {"app_name": app_name, "resolved_channel": channel, "interference_level": "global_takeover", "took_focus": True, "fallback_count": fallback_count, "target_focused": surface_has_focus(target), "detail": f"sent global {key_name} to app"}
        fallback_count += 1
    raise SystemExit(f"surface action failed for {action}")


def list_surfaces(payload):
    app_name = payload["app_name"]
    channel = payload["channel"]
    pid = payload.get("pid")
    instance_id = payload["instance_id"]
    _, surfaces, _ = collect_surfaces(app_name, channel, pid, instance_id)
    return {"app_name": app_name, "surfaces": surfaces}


def snapshot_surfaces(payload):
    app_name = payload["app_name"]
    channel = payload["channel"]
    pid = payload.get("pid")
    instance_id = payload["instance_id"]
    _, surfaces, path_map = collect_surfaces(app_name, channel, pid, instance_id)
    root_surface_id = payload.get("root_surface_id")
    if root_surface_id:
        _, root_path = find_surface(path_map, root_surface_id)
        surfaces = [item for item in surfaces if item["path"] == root_path or item["path"].startswith(root_path + "/")]
        if not surfaces:
            raise SystemExit(f"surface not found: {root_surface_id}")
    window_title = None
    if surfaces:
        for item in surfaces:
            if item.get("window_title"):
                window_title = item["window_title"]
                break
    capture = capture_window(app_name, window_title)
    return {"app_name": app_name, "surfaces": surfaces, **capture}


def main():
    if len(sys.argv) < 3:
        raise SystemExit("mode and payload json are required")
    mode = sys.argv[1]
    payload = json.loads(sys.argv[2])
    if mode == "status":
        result = read_status(payload)
    elif mode == "request_service":
        result = request_service(payload)
    elif mode == "surface_list":
        result = list_surfaces(payload)
    elif mode == "surface_snapshot":
        result = snapshot_surfaces(payload)
    elif mode == "surface_action":
        result = surface_action(payload)
    else:
        raise SystemExit(f"unsupported bridge mode: {mode}")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
"#;

pub fn host_access_status() -> Result<HostAccessStatus> {
    #[cfg(target_os = "macos")]
    {
        return active_host_access_status()
            .or_else(|error| Ok(unavailable_macos_host_access_status(&error.to_string())));
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(HostAccessStatus {
            platform: std::env::consts::OS.to_string(),
            app_targets: Vec::new(),
            services: default_services()
                .into_iter()
                .map(|service| HostAccessProbe {
                    service,
                    state: PermissionState::Unsupported,
                    requestable: false,
                    open_settings_url: None,
                    detail: "macOS-native controls are unavailable on this platform".to_string(),
                })
                .collect(),
            execution_channels: vec![ExecutionChannelAvailability {
                channel: ExecutionChannel::Cdp,
                available: true,
                interference_level: InterferenceLevel::BackgroundSafe,
                detail: "CDP remains available outside macOS-native controls.".to_string(),
            }],
            assistive_overlays: Vec::new(),
            recommended_services: Vec::new(),
            summary: "macOS-native host access is unsupported on this platform".to_string(),
        })
    }
}

#[cfg(target_os = "macos")]
fn active_host_access_status() -> Result<HostAccessStatus> {
    let bridge: BridgeStatusPayload = serde_json::from_value(run_bridge_json("status", json!({}))?)
        .context("deserialize host access status bridge payload")?;
    let settings = settings_manifest()?;
    let overlays = assistive_overlay_manifest()?.overlays;
    let mut services = bridge.services;
    services.push(HostAccessProbe {
        service: HostAccessService::DevtoolsSecurity,
        state: if devtools_security_enabled() {
            PermissionState::Granted
        } else {
            PermissionState::Missing
        },
        requestable: true,
        open_settings_url: None,
        detail: command_output("DevToolsSecurity", &["-status"]),
    });
    for probe in &mut services {
        if probe.open_settings_url.is_none() {
            probe.open_settings_url = settings
                .services
                .iter()
                .find(|candidate| candidate.service == probe.service)
                .map(|candidate| candidate.open_settings_url.clone());
        }
    }
    let mut status = HostAccessStatus {
        platform: "macos".to_string(),
        app_targets: vec![
            "Google Chrome Dev".to_string(),
            "Google Chrome".to_string(),
            "Chromium".to_string(),
        ],
        services,
        execution_channels: Vec::new(),
        assistive_overlays: overlays,
        recommended_services: Vec::new(),
        summary: String::new(),
    };
    refresh_host_access_status(&mut status);
    Ok(status)
}

pub fn host_access_setup(request: &HostAccessSetupRequest) -> Result<HostAccessSetupResult> {
    let before = host_access_status()?;
    let settings = settings_manifest()?;
    let services = if request.services.is_empty() {
        default_services()
    } else {
        request.services.clone()
    };
    let mut steps = Vec::new();
    for service in services {
        let before_probe = find_probe(&before, &service)
            .cloned()
            .unwrap_or(HostAccessProbe {
                service: service.clone(),
                state: PermissionState::Unknown,
                requestable: false,
                open_settings_url: settings
                    .services
                    .iter()
                    .find(|candidate| candidate.service == service)
                    .map(|candidate| candidate.open_settings_url.clone()),
                detail: "service probe missing from host access status".to_string(),
            });
        if before_probe.state == PermissionState::Granted {
            steps.push(HostAccessSetupStep {
                service,
                action: "already_granted".to_string(),
                ok: true,
                state: PermissionState::Granted,
                detail: before_probe.detail,
                opened_settings: false,
            });
            continue;
        }
        if request.mode == HostAccessSetupMode::Audit {
            steps.push(HostAccessSetupStep {
                service,
                action: "audit_only".to_string(),
                ok: before_probe.state == PermissionState::Granted,
                state: before_probe.state,
                detail: before_probe.detail,
                opened_settings: false,
            });
            continue;
        }
        let (mut state, mut detail) = request_service(&service)?;
        let mut opened_settings = false;
        if state != PermissionState::Granted
            && request.open_settings_on_missing
            && let Some(url) = settings
                .services
                .iter()
                .find(|candidate| candidate.service == service)
                .map(|candidate| candidate.open_settings_url.as_str())
        {
            let status = Command::new("open")
                .arg("-g")
                .arg(url)
                .status()
                .with_context(|| format!("open settings url for {}", service.as_str()))?;
            opened_settings = status.success();
            if opened_settings {
                detail = format!("{detail}; opened settings");
            }
        }
        if service == HostAccessService::DevtoolsSecurity {
            state = if devtools_security_enabled() {
                PermissionState::Granted
            } else {
                PermissionState::Missing
            };
        }
        steps.push(HostAccessSetupStep {
            service,
            action: "apply".to_string(),
            ok: state == PermissionState::Granted,
            state,
            detail,
            opened_settings,
        });
    }
    let mut after = host_access_status()?;
    apply_setup_steps(&mut after, &steps);
    let changed_services = after
        .services
        .iter()
        .filter_map(|after_probe| {
            let before_state = find_probe(&before, &after_probe.service)
                .map(|probe| probe.state.clone())
                .unwrap_or(PermissionState::Unknown);
            (before_state != after_probe.state).then_some(after_probe.service.clone())
        })
        .collect();
    Ok(HostAccessSetupResult {
        mode: request.mode.clone(),
        before,
        after,
        steps,
        changed_services,
    })
}

pub fn browser_surface_list(instance: &BrowserInstance) -> Result<BrowserSurfaceListPayload> {
    let payload: BridgeSurfacePayload = serde_json::from_value(run_bridge_json(
        "surface_list",
        json!({
            "app_name": browser_app_name(instance),
            "pid": instance.pid,
            "instance_id": instance.id,
            "channel": instance.channel.as_str(),
        }),
    )?)
    .context("deserialize browser surface list payload")?;
    Ok(BrowserSurfaceListPayload {
        instance: instance.clone(),
        app_name: payload.app_name,
        surfaces: attach_bundle_id(instance, payload.surfaces),
    })
}

pub fn browser_surface_snapshot(
    instance: &BrowserInstance,
    root_surface_id: Option<&str>,
) -> Result<BrowserSurfaceSnapshotResult> {
    let payload: NativeSurfaceSnapshot = serde_json::from_value(run_bridge_json(
        "surface_snapshot",
        json!({
            "app_name": browser_app_name(instance),
            "pid": instance.pid,
            "instance_id": instance.id,
            "channel": instance.channel.as_str(),
            "root_surface_id": root_surface_id,
        }),
    )?)
    .context("deserialize browser surface snapshot payload")?;
    let surfaces = attach_bundle_id(instance, payload.surfaces);
    ensure_snapshot_scope(root_surface_id, &surfaces)?;
    Ok(BrowserSurfaceSnapshotResult {
        app_name: payload.app_name,
        surfaces,
        capture_path: payload.capture_path,
        capture_source: payload.capture_source,
        capture_detail: payload.capture_detail,
    })
}

pub fn browser_surface_action(
    instance: &BrowserInstance,
    request: &BrowserSurfaceActionRequest,
) -> Result<BrowserSurfaceActionPayload> {
    let payload: BridgeActionPayload = serde_json::from_value(run_bridge_json(
        "surface_action",
        json!({
            "app_name": browser_app_name(instance),
            "pid": instance.pid,
            "instance_id": instance.id,
            "channel": instance.channel.as_str(),
            "request": request,
        }),
    )?)
    .context("deserialize browser surface action payload")?;
    ensure_focus_result(request, &payload)?;
    Ok(BrowserSurfaceActionPayload {
        instance: instance.clone(),
        app_name: payload.app_name,
        target_surface_id: request.surface_id.clone(),
        requested: request.clone(),
        resolved_channel: payload.resolved_channel,
        interference_level: payload.interference_level,
        took_focus: payload.took_focus,
        fallback_count: payload.fallback_count,
        detail: payload.detail,
    })
}

pub fn capture_bytes_from_snapshot(
    result: &BrowserSurfaceSnapshotResult,
) -> Result<Option<Vec<u8>>> {
    let Some(path) = result.capture_path.as_ref() else {
        return Ok(None);
    };
    let bytes = fs::read(path).with_context(|| format!("read native capture {}", path))?;
    let _ = fs::remove_file(path);
    Ok(Some(bytes))
}

pub fn cleanup_snapshot_capture(result: &BrowserSurfaceSnapshotResult) {
    if let Some(path) = result.capture_path.as_ref() {
        let _ = fs::remove_file(path);
    }
}

fn browser_app_name(instance: &BrowserInstance) -> String {
    match instance.channel.as_str() {
        "chrome-dev" => "Google Chrome Dev".to_string(),
        "chrome" => "Google Chrome".to_string(),
        "chromium" => "Chromium".to_string(),
        _ => "Google Chrome Dev".to_string(),
    }
}

fn browser_bundle_id(instance: &BrowserInstance) -> &'static str {
    match instance.channel.as_str() {
        "chrome-dev" => "com.google.Chrome.dev",
        "chrome" => "com.google.Chrome",
        "chromium" => "org.chromium.Chromium",
        _ => "com.google.Chrome.dev",
    }
}

fn attach_bundle_id(
    instance: &BrowserInstance,
    mut surfaces: Vec<BrowserSurfaceDescriptor>,
) -> Vec<BrowserSurfaceDescriptor> {
    let bundle_id = Some(browser_bundle_id(instance).to_string());
    for surface in &mut surfaces {
        surface.bundle_id = bundle_id.clone();
    }
    surfaces
}

fn default_services() -> Vec<HostAccessService> {
    DEFAULT_SETUP_SERVICES.to_vec()
}

#[cfg(target_os = "macos")]
fn unavailable_macos_host_access_status(detail: &str) -> HostAccessStatus {
    let settings = settings_manifest().ok();
    let overlays = assistive_overlay_manifest()
        .map(|manifest| manifest.overlays)
        .unwrap_or_default();
    let services = default_services()
        .into_iter()
        .map(|service| {
            let open_settings_url = settings.as_ref().and_then(|manifest| {
                manifest
                    .services
                    .iter()
                    .find(|candidate| candidate.service == service)
                    .map(|candidate| candidate.open_settings_url.clone())
            });
            let probe_detail = unavailable_probe_detail(&service, detail);
            HostAccessProbe {
                service,
                state: PermissionState::Unknown,
                requestable: false,
                open_settings_url,
                detail: probe_detail,
            }
        })
        .collect();
    let mut status = HostAccessStatus {
        platform: "macos".to_string(),
        app_targets: vec![
            "Google Chrome Dev".to_string(),
            "Google Chrome".to_string(),
            "Chromium".to_string(),
        ],
        services,
        execution_channels: Vec::new(),
        assistive_overlays: overlays,
        recommended_services: Vec::new(),
        summary: String::new(),
    };
    refresh_host_access_status(&mut status);
    status.summary = format!("{}; macOS control bridge unavailable", status.summary);
    status
}

#[cfg(target_os = "macos")]
fn unavailable_probe_detail(service: &HostAccessService, detail: &str) -> String {
    match service {
        HostAccessService::AppleEventsChrome
        | HostAccessService::AppleEventsChromeDev
        | HostAccessService::AppleEventsChromium => format!(
            "Read-only diagnostics do not probe Automation consent because the macOS control bridge is unavailable: {detail}"
        ),
        _ => format!("macOS control bridge unavailable: {detail}"),
    }
}

fn assistive_overlay_manifest() -> Result<AssistiveOverlayManifest> {
    serde_json::from_str(include_str!(
        "../../../reference/platform/macos/assistive-overlays.json"
    ))
    .context("parse assistive overlay manifest")
}

fn settings_manifest() -> Result<SettingsManifest> {
    serde_json::from_str(include_str!(
        "../../../reference/platform/macos/system-settings-deeplinks.json"
    ))
    .context("parse system settings manifest")
}

fn run_bridge_json(mode: &str, payload: Value) -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("python3")
            .arg("-c")
            .arg(BRIDGE_SCRIPT)
            .arg(mode)
            .arg(payload.to_string())
            .output()
            .context("run macOS control bridge")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit status {}", output.status)
            };
            bail!("{detail}");
        }
        serde_json::from_slice(&output.stdout).context("parse macOS control bridge JSON")
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (mode, payload);
        bail!("macOS control bridge is unsupported on this platform")
    }
}

fn request_service(service: &HostAccessService) -> Result<(PermissionState, String)> {
    #[cfg(target_os = "macos")]
    {
        if *service == HostAccessService::DevtoolsSecurity {
            let status = Command::new("sudo")
                .arg("-n")
                .arg("DevToolsSecurity")
                .arg("-enable")
                .status()
                .context("request DevToolsSecurity enable")?;
            return Ok((
                if status.success() && devtools_security_enabled() {
                    PermissionState::Granted
                } else {
                    PermissionState::Missing
                },
                "DevToolsSecurity enable requested".to_string(),
            ));
        }
        let value = run_bridge_json(
            "request_service",
            json!({
                "service": service,
                "app_name": match service {
                    HostAccessService::AppleEventsChrome => Some("Google Chrome"),
                    HostAccessService::AppleEventsChromeDev => Some("Google Chrome Dev"),
                    HostAccessService::AppleEventsChromium => Some("Chromium"),
                    _ => None,
                }
            }),
        )?;
        let state: PermissionState = serde_json::from_value(
            value
                .get("state")
                .cloned()
                .ok_or_else(|| anyhow!("request_service bridge did not return state"))?,
        )
        .context("parse request_service state")?;
        let detail = value
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("host access request completed")
            .to_string();
        Ok((state, detail))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = service;
        Ok((
            PermissionState::Unsupported,
            "host access setup is unsupported on this platform".to_string(),
        ))
    }
}

fn find_probe<'a>(
    status: &'a HostAccessStatus,
    service: &HostAccessService,
) -> Option<&'a HostAccessProbe> {
    status
        .services
        .iter()
        .find(|probe| &probe.service == service)
}

fn service_granted(statuses: &[HostAccessProbe], service: HostAccessService) -> bool {
    statuses
        .iter()
        .find(|probe| probe.service == service)
        .map(|probe| probe.state == PermissionState::Granted)
        .unwrap_or(false)
}

fn automation_granted(statuses: &[HostAccessProbe]) -> bool {
    [
        HostAccessService::AppleEventsChrome,
        HostAccessService::AppleEventsChromeDev,
        HostAccessService::AppleEventsChromium,
    ]
    .into_iter()
    .any(|service| service_granted(statuses, service))
}

fn automation_target_label(service: &HostAccessService) -> Option<&'static str> {
    match service {
        HostAccessService::AppleEventsChrome => Some("Google Chrome"),
        HostAccessService::AppleEventsChromeDev => Some("Google Chrome Dev"),
        HostAccessService::AppleEventsChromium => Some("Chromium"),
        _ => None,
    }
}

fn automation_targets_by_state(statuses: &[HostAccessProbe], granted: bool) -> Vec<&'static str> {
    statuses
        .iter()
        .filter(|probe| (probe.state == PermissionState::Granted) == granted)
        .filter_map(|probe| automation_target_label(&probe.service))
        .collect()
}

fn refresh_host_access_status(status: &mut HostAccessStatus) {
    let accessibility_ok = service_granted(&status.services, HostAccessService::Accessibility);
    let listen_ok = service_granted(&status.services, HostAccessService::ListenEvent);
    let apple_ok = automation_granted(&status.services);
    let granted_targets = automation_targets_by_state(&status.services, true);
    let unverified_targets = automation_targets_by_state(&status.services, false);
    let granted_targets_detail = if granted_targets.is_empty() {
        "none".to_string()
    } else {
        granted_targets.join(", ")
    };
    let unverified_targets_detail = if unverified_targets.is_empty() {
        "none".to_string()
    } else {
        unverified_targets.join(", ")
    };
    let (system_events_ok, system_events_detail) = system_events_probe();
    let takeover_ok = listen_ok && apple_ok && system_events_ok;
    let total_services = status.services.len();
    status.execution_channels = vec![
        ExecutionChannelAvailability {
            channel: ExecutionChannel::Cdp,
            available: true,
            interference_level: InterferenceLevel::BackgroundSafe,
            detail: "Primary meshing channel for page and tab control.".to_string(),
        },
        ExecutionChannelAvailability {
            channel: ExecutionChannel::AxDirect,
            available: accessibility_ok,
            interference_level: InterferenceLevel::BackgroundSafe,
            detail: "Direct macOS Accessibility action and discovery path.".to_string(),
        },
        ExecutionChannelAvailability {
            channel: ExecutionChannel::AppleEventsActivation,
            available: apple_ok,
            interference_level: InterferenceLevel::AppTakeover,
            detail: if apple_ok {
                format!(
                    "Automation fallback is currently ready for: {granted_targets_detail}. Remaining app-target states are unverified or missing for: {unverified_targets_detail}."
                )
            } else {
                format!(
                    "Automation fallback is app-target specific and currently unverified for: {unverified_targets_detail}. Read-only diagnostics do not probe Automation consent."
                )
            },
        },
        ExecutionChannelAvailability {
            channel: ExecutionChannel::AppScopedKeyPost,
            available: accessibility_ok,
            interference_level: InterferenceLevel::BackgroundSafe,
            detail: "App-scoped key posting through Accessibility.".to_string(),
        },
        ExecutionChannelAvailability {
            channel: ExecutionChannel::GlobalTakeover,
            available: takeover_ok,
            interference_level: InterferenceLevel::GlobalTakeover,
            detail: if takeover_ok {
                format!(
                    "System Events takeover is ready for granted targets: {granted_targets_detail}."
                )
            } else {
                format!(
                    "System Events takeover requires Listen Event, a granted per-app Apple Events permission, and verified System Events automation. {system_events_detail}"
                )
            },
        },
    ];
    status.recommended_services = status
        .services
        .iter()
        .filter(|probe| {
            probe.state != PermissionState::Granted && probe.state != PermissionState::Unsupported
        })
        .map(|probe| probe.service.clone())
        .collect();
    let granted_count = status
        .services
        .iter()
        .filter(|probe| probe.state == PermissionState::Granted)
        .count();
    status.summary = format!(
        "{granted_count} of {} host access services are currently granted",
        total_services
    );
}

fn apply_setup_steps(status: &mut HostAccessStatus, steps: &[HostAccessSetupStep]) {
    for step in steps {
        if let Some(probe) = status
            .services
            .iter_mut()
            .find(|probe| probe.service == step.service)
        {
            probe.state = step.state.clone();
            probe.detail = step.detail.clone();
        } else {
            status.services.push(HostAccessProbe {
                service: step.service.clone(),
                state: step.state.clone(),
                requestable: true,
                open_settings_url: None,
                detail: step.detail.clone(),
            });
        }
    }
    refresh_host_access_status(status);
}

fn command_output(binary: &str, args: &[&str]) -> String {
    match Command::new(binary).args(args).output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if detail.is_empty() {
                "command failed".to_string()
            } else {
                detail
            }
        }
        Err(error) => format!("{error}"),
    }
}

fn devtools_security_enabled() -> bool {
    command_output("DevToolsSecurity", &["-status"])
        .to_ascii_lowercase()
        .contains("enabled")
}

fn system_events_probe() -> (bool, String) {
    #[cfg(target_os = "macos")]
    {
        (
            false,
            "Read-only diagnostics do not probe System Events automation to avoid consent prompts"
                .to_string(),
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        (
            false,
            "System Events is unavailable on this platform".to_string(),
        )
    }
}

fn ensure_snapshot_scope(
    root_surface_id: Option<&str>,
    surfaces: &[BrowserSurfaceDescriptor],
) -> Result<()> {
    if let Some(root_surface_id) = root_surface_id
        && surfaces.is_empty()
    {
        bail!("surface not found: {root_surface_id}");
    }
    Ok(())
}

fn ensure_focus_result(
    request: &BrowserSurfaceActionRequest,
    payload: &BridgeActionPayload,
) -> Result<()> {
    if request.action == pengu_mesh_shared::SurfaceActionKind::Focus && !payload.target_focused {
        let target = request.surface_id.as_deref().unwrap_or("ax:0");
        bail!("surface action failed to focus target: {target}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BridgeActionPayload, HostAccessStatus, apply_setup_steps, assistive_overlay_manifest,
        attach_bundle_id, automation_granted, browser_bundle_id, ensure_focus_result,
        ensure_snapshot_scope, refresh_host_access_status, settings_manifest, system_events_probe,
    };
    use pengu_mesh_shared::{
        BrowserChannel, BrowserInstance, BrowserSurfaceActionRequest, BrowserSurfaceDescriptor,
        ExecutionChannel, HostAccessProbe, HostAccessService, InstanceMode, InstanceStatus,
        InterferenceLevel, PermissionState, SurfaceActionKind,
    };

    #[test]
    fn overlay_manifest_parses() {
        let overlays = assistive_overlay_manifest().expect("assistive overlay manifest");
        assert!(!overlays.overlays.is_empty());
    }

    #[test]
    fn settings_manifest_parses() {
        let settings = settings_manifest().expect("settings manifest");
        assert!(!settings.services.is_empty());
    }

    #[test]
    fn automation_granted_accounts_for_chromium() {
        let statuses = vec![HostAccessProbe {
            service: HostAccessService::AppleEventsChromium,
            state: PermissionState::Granted,
            requestable: true,
            open_settings_url: None,
            detail: "granted".to_string(),
        }];
        assert!(automation_granted(&statuses));
    }

    #[test]
    fn snapshot_scope_rejects_empty_surface_results_for_explicit_root() {
        let error = ensure_snapshot_scope(Some("ax:0/9"), &[]).expect_err("missing root");
        assert!(error.to_string().contains("surface not found: ax:0/9"));
    }

    #[test]
    fn focus_result_rejects_false_success_payloads() {
        let request = BrowserSurfaceActionRequest {
            surface_id: Some("ax:0/4".to_string()),
            action: SurfaceActionKind::Focus,
            value: None,
            key_sequence: None,
            execution_channel: Some(ExecutionChannel::AppleEventsActivation),
            allow_takeover: None,
        };
        let payload = BridgeActionPayload {
            app_name: "Google Chrome Dev".to_string(),
            resolved_channel: ExecutionChannel::AppleEventsActivation,
            interference_level: InterferenceLevel::AppTakeover,
            took_focus: true,
            fallback_count: 1,
            target_focused: false,
            detail: "activated app for focus".to_string(),
        };
        let error = ensure_focus_result(&request, &payload).expect_err("false focus success");
        assert!(
            error
                .to_string()
                .contains("surface action failed to focus target")
        );
    }

    #[test]
    fn system_events_status_probe_is_read_only() {
        let (available, detail) = system_events_probe();
        assert!(!available);
        assert!(detail.contains("do not probe System Events automation"));
    }

    #[test]
    fn browser_bundle_id_matches_browser_channel() {
        assert_eq!(
            browser_bundle_id(&test_browser_instance(BrowserChannel::ChromeDev)),
            "com.google.Chrome.dev"
        );
        assert_eq!(
            browser_bundle_id(&test_browser_instance(BrowserChannel::Chrome)),
            "com.google.Chrome"
        );
        assert_eq!(
            browser_bundle_id(&test_browser_instance(BrowserChannel::Chromium)),
            "org.chromium.Chromium"
        );
    }

    #[test]
    fn attach_bundle_id_stamps_every_surface() {
        let surfaces = attach_bundle_id(
            &test_browser_instance(BrowserChannel::ChromeDev),
            vec![
                BrowserSurfaceDescriptor {
                    id: "ax:0".to_string(),
                    parent_id: None,
                    path: "0".to_string(),
                    role: "AXApplication".to_string(),
                    title: Some("Google Chrome Dev".to_string()),
                    description: None,
                    value: None,
                    window_title: Some("Window".to_string()),
                    actions: vec!["focus".to_string()],
                    focused: true,
                    enabled: true,
                    app_name: "Google Chrome Dev".to_string(),
                    bundle_id: None,
                    channel: BrowserChannel::ChromeDev,
                    instance_id: "inst_demo".to_string(),
                },
                BrowserSurfaceDescriptor {
                    id: "ax:0/1".to_string(),
                    parent_id: Some("ax:0".to_string()),
                    path: "0/1".to_string(),
                    role: "AXButton".to_string(),
                    title: Some("Continue".to_string()),
                    description: None,
                    value: None,
                    window_title: Some("Window".to_string()),
                    actions: vec!["press".to_string()],
                    focused: false,
                    enabled: true,
                    app_name: "Google Chrome Dev".to_string(),
                    bundle_id: Some("stale.bundle".to_string()),
                    channel: BrowserChannel::ChromeDev,
                    instance_id: "inst_demo".to_string(),
                },
            ],
        );

        assert!(
            surfaces
                .iter()
                .all(|surface| { surface.bundle_id.as_deref() == Some("com.google.Chrome.dev") })
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn host_access_status_reports_chromium_and_read_only_automation_detail() {
        let status = super::host_access_status().expect("host access status");
        let chromium = status
            .services
            .iter()
            .find(|probe| probe.service == HostAccessService::AppleEventsChromium)
            .expect("chromium automation probe");
        assert_eq!(chromium.state, PermissionState::Unknown);
        assert!(chromium.detail.contains("do not probe Automation"));
    }

    #[test]
    fn refresh_host_access_status_mentions_granted_and_unverified_targets() {
        let mut status = HostAccessStatus {
            platform: "macos".to_string(),
            app_targets: vec![
                "Google Chrome Dev".to_string(),
                "Google Chrome".to_string(),
                "Chromium".to_string(),
            ],
            services: vec![
                HostAccessProbe {
                    service: HostAccessService::Accessibility,
                    state: PermissionState::Granted,
                    requestable: true,
                    open_settings_url: None,
                    detail: "granted".to_string(),
                },
                HostAccessProbe {
                    service: HostAccessService::ListenEvent,
                    state: PermissionState::Granted,
                    requestable: true,
                    open_settings_url: None,
                    detail: "granted".to_string(),
                },
                HostAccessProbe {
                    service: HostAccessService::AppleEventsChromium,
                    state: PermissionState::Granted,
                    requestable: true,
                    open_settings_url: None,
                    detail: "granted".to_string(),
                },
                HostAccessProbe {
                    service: HostAccessService::AppleEventsChrome,
                    state: PermissionState::Unknown,
                    requestable: true,
                    open_settings_url: None,
                    detail: "unverified".to_string(),
                },
            ],
            execution_channels: Vec::new(),
            assistive_overlays: Vec::new(),
            recommended_services: Vec::new(),
            summary: String::new(),
        };
        refresh_host_access_status(&mut status);
        let activation = status
            .execution_channels
            .iter()
            .find(|channel| channel.channel == ExecutionChannel::AppleEventsActivation)
            .expect("automation channel");
        assert!(activation.available);
        assert!(activation.detail.contains("Chromium"));
        assert!(activation.detail.contains("Google Chrome"));
    }

    #[test]
    fn apply_setup_steps_updates_after_status_for_chromium() {
        let mut status = HostAccessStatus {
            platform: "macos".to_string(),
            app_targets: vec![
                "Google Chrome Dev".to_string(),
                "Google Chrome".to_string(),
                "Chromium".to_string(),
            ],
            services: vec![HostAccessProbe {
                service: HostAccessService::AppleEventsChromium,
                state: PermissionState::Unknown,
                requestable: true,
                open_settings_url: None,
                detail: "Read-only diagnostics do not probe Automation".to_string(),
            }],
            execution_channels: Vec::new(),
            assistive_overlays: Vec::new(),
            recommended_services: Vec::new(),
            summary: String::new(),
        };
        apply_setup_steps(
            &mut status,
            &[pengu_mesh_shared::HostAccessSetupStep {
                service: HostAccessService::AppleEventsChromium,
                action: "apply".to_string(),
                ok: true,
                state: PermissionState::Granted,
                detail: "Apple Events probe succeeded".to_string(),
                opened_settings: false,
            }],
        );
        let chromium = status
            .services
            .iter()
            .find(|probe| probe.service == HostAccessService::AppleEventsChromium)
            .expect("chromium service");
        assert_eq!(chromium.state, PermissionState::Granted);
        let activation = status
            .execution_channels
            .iter()
            .find(|channel| channel.channel == ExecutionChannel::AppleEventsActivation)
            .expect("automation channel");
        assert!(activation.available);
        assert!(activation.detail.contains("Chromium"));
    }

    fn test_browser_instance(channel: BrowserChannel) -> BrowserInstance {
        BrowserInstance {
            id: format!("inst_{}", channel.as_str()),
            name: format!("{} instance", channel.as_str()),
            channel,
            mode: InstanceMode::Attached,
            status: InstanceStatus::Attached,
            debug_http_url: "http://127.0.0.1:9222".to_string(),
            browser_ws_url: Some("ws://127.0.0.1:9222/devtools/browser/demo".to_string()),
            profile_id: None,
            profile_path: None,
            pid: Some(4242),
            last_error: None,
            created_at: "2026-03-12T00:00:00Z".to_string(),
            updated_at: "2026-03-12T00:00:00Z".to_string(),
        }
    }
}
