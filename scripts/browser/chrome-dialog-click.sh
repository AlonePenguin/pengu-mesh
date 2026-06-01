#!/bin/zsh
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  chrome-dialog-click.sh --list [app_name]
  chrome-dialog-click.sh <button_label> [app_name] [attempts] [delay_seconds]

Examples:
  ./scripts/browser/chrome-dialog-click.sh --list
  ./scripts/browser/chrome-dialog-click.sh Allow
  ./scripts/browser/chrome-dialog-click.sh Reload "Google Chrome Dev" 40 0.1
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ "${1:-}" == "--list" ]]; then
  app_name="${2:-Google Chrome Dev}"
  python3 - "$app_name" <<'PY'
import subprocess
import sys
from AppKit import NSWorkspace
from ApplicationServices import (
    AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication,
    kAXChildrenAttribute,
    kAXDescriptionAttribute,
    kAXRoleAttribute,
    kAXTitleAttribute,
    kAXValueAttribute,
)


def activate(app_name: str) -> None:
    subprocess.run(
        ["osascript", "-e", f'tell application "{app_name}" to activate'],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def get_pid(app_name: str) -> int:
    for app in NSWorkspace.sharedWorkspace().runningApplications():
        if app.localizedName() == app_name:
            return app.processIdentifier()
    raise SystemExit(f"app not running: {app_name}")


def attr(element, name):
    err, value = AXUIElementCopyAttributeValue(element, name, None)
    return value if err == 0 else None


def children(element):
    value = attr(element, kAXChildrenAttribute)
    return list(value) if value else []


def button_labels(element) -> list[str]:
    labels = []
    for key in (kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute):
        value = attr(element, key)
        if isinstance(value, str) and value.strip():
            labels.append(value.strip())
    return labels


def is_pressable(element) -> bool:
    return attr(element, kAXRoleAttribute) in {"AXButton", "AXCheckBox", "AXRadioButton"}


def walk(element):
    queue = [element]
    while queue:
        current = queue.pop(0)
        yield current
        queue.extend(children(current))


app_name = sys.argv[1]
activate(app_name)
app = AXUIElementCreateApplication(get_pid(app_name))
labels = []
seen = set()
for element in walk(app):
    if not is_pressable(element):
        continue
    for label in button_labels(element):
        if label in seen:
            continue
        seen.add(label)
        labels.append(label)

for label in labels:
    print(label)
PY
  exit 0
fi

button_label="${1:-}"
app_name="${2:-Google Chrome Dev}"
attempts="${3:-50}"
delay_seconds="${4:-0.2}"

if [[ -z "$button_label" ]]; then
  usage >&2
  exit 2
fi

python3 - "$app_name" "$button_label" "$attempts" "$delay_seconds" <<'PY'
import subprocess
import sys
import time
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
    kAXValueAttribute,
)


def activate(app_name: str) -> None:
    subprocess.run(
        ["osascript", "-e", f'tell application "{app_name}" to activate'],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def get_pid(app_name: str) -> int:
    for app in NSWorkspace.sharedWorkspace().runningApplications():
        if app.localizedName() == app_name:
            return app.processIdentifier()
    raise SystemExit(f"app not running: {app_name}")


def attr(element, name):
    err, value = AXUIElementCopyAttributeValue(element, name, None)
    return value if err == 0 else None


def children(element):
    value = attr(element, kAXChildrenAttribute)
    return list(value) if value else []


def is_pressable(element) -> bool:
    return attr(element, kAXRoleAttribute) in {"AXButton", "AXCheckBox", "AXRadioButton"}


def button_matches(element, target: str) -> bool:
    needle = target.casefold()
    for key in (kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute):
        value = attr(element, key)
        if isinstance(value, str):
            normalized = value.strip().casefold()
            if normalized and (normalized == needle or needle in normalized):
                return True
    return False


def default_accept(app_name: str) -> bool:
    result = subprocess.run(
        [
            "osascript",
            "-",
            app_name,
        ],
        input="""
on run argv
  set appName to item 1 of argv
  tell application appName to activate
  delay 0.2
  tell application "System Events"
    key code 36
  end tell
end run
""",
        text=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def button_still_visible(app_name: str, target: str) -> bool:
    return find_button(app_name, target) is not None


def can_fallback_to_return(target: str) -> bool:
    return target.strip().casefold() in {"allow", "ok", "open", "continue"}


def find_button(app_name: str, target: str):
    app = AXUIElementCreateApplication(get_pid(app_name))
    for element in walk(app):
        if not is_pressable(element):
            continue
        if button_matches(element, target):
            return element
    return None


def walk(element):
    queue = [element]
    while queue:
        current = queue.pop(0)
        yield current
        queue.extend(children(current))


app_name = sys.argv[1]
target = sys.argv[2]
attempts = int(sys.argv[3])
delay_seconds = float(sys.argv[4])

activate(app_name)
for _ in range(attempts):
    button = find_button(app_name, target)
    if button is not None:
        err = AXUIElementPerformAction(button, kAXPressAction)
        if err == 0:
            print(f"clicked:{target}")
            raise SystemExit(0)
    elif can_fallback_to_return(target) and default_accept(app_name):
        time.sleep(delay_seconds)
        if not button_still_visible(app_name, target):
            print(f"clicked:{target}:return-fallback")
            raise SystemExit(0)
    time.sleep(delay_seconds)

raise SystemExit(f"button not found: {target}")
PY
