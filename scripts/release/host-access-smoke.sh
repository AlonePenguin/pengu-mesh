#!/bin/zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  cargo_bin="${HOME}/.cargo/bin/cargo"
else
  cargo_bin="$(command -v cargo)"
fi

output_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/pengu-mesh-host-access-smoke.XXXXXX")}"
mkdir -p "$output_dir"

status_json="${output_dir}/host-access-status.json"
audit_json="${output_dir}/host-access-setup-audit.json"
apply_json="${output_dir}/host-access-setup-apply.json"
verification_json="${output_dir}/verification.json"

"${cargo_bin}" run --quiet -p pengu-mesh -- host-access-status > "${status_json}"
"${cargo_bin}" run --quiet -p pengu-mesh -- host-access-setup --mode audit > "${audit_json}"

missing_service="$(
  /usr/bin/python3 - "${status_json}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

services = payload["data"]["services"]
preferred = [
    service["service"]
    for service in services
    if service["state"] != "granted" and service["service"] != "devtools_security"
]
fallback = [
    service["service"]
    for service in services
    if service["state"] != "granted"
]
selected = preferred[0] if preferred else (fallback[0] if fallback else "")
print(selected)
PY
)"

apply_mode="steady_state_only"
if [[ -n "${missing_service}" ]]; then
  PENGU_MESH_CAPABILITY_GRANTS=host_access_setup \
    "${cargo_bin}" run --quiet -p pengu-mesh -- host-access-setup --mode apply --service "${missing_service}" --open-settings-on-missing > "${apply_json}"
  apply_mode="apply:${missing_service}"
fi

/usr/bin/python3 - "${status_json}" "${audit_json}" "${apply_json}" "${missing_service}" "${verification_json}" <<'PY'
import json
import os
import sys

status_path, audit_path, apply_path, missing_service, verification_path = sys.argv[1:6]


def load(path):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


status_payload = load(status_path)
audit_payload = load(audit_path)

assert status_payload["ok"] is True, status_payload
assert audit_payload["ok"] is True, audit_payload

status = status_payload["data"]
audit = audit_payload["data"]
services = status["services"]

assert status["platform"], status
assert len(services) >= 5, services
assert all("service" in probe and "state" in probe for probe in services), services
assert all(probe.get("open_settings_url") for probe in services), services
assert audit["mode"] == "audit", audit
assert len(audit["steps"]) == len(audit["before"]["services"]), audit["steps"]

missing_services = [probe["service"] for probe in services if probe["state"] != "granted"]
granted_count = sum(1 for probe in services if probe["state"] == "granted")

result = {
    "platform": status["platform"],
    "service_count": len(services),
    "granted_count": granted_count,
    "missing_services": missing_services,
    "all_have_settings_urls": all(bool(probe.get("open_settings_url")) for probe in services),
    "audit_step_actions": sorted({step["action"] for step in audit["steps"]}),
    "apply_mode": "steady_state_only",
}

if missing_service:
    apply_payload = load(apply_path)
    assert apply_payload["ok"] is True, apply_payload
    apply_data = apply_payload["data"]
    assert apply_data["mode"] == "apply", apply_data
    matching_steps = [step for step in apply_data["steps"] if step["service"] == missing_service]
    assert matching_steps, apply_data["steps"]
    step = matching_steps[0]
    assert "opened_settings" in step, step
    assert step["action"] in {"apply", "already_granted"}, step
    result["apply_mode"] = f"apply:{missing_service}"
    result["apply_step"] = step
else:
    assert os.path.exists(status_path), status_path

with open(verification_path, "w", encoding="utf-8") as handle:
    json.dump(result, handle, indent=2, sort_keys=True)
PY

cat > "${output_dir}/summary.md" <<EOF
# Host Access Smoke

- output_dir: ${output_dir}
- status_payload: ${status_json}
- audit_payload: ${audit_json}
- verification_payload: ${verification_json}
- apply_mode: ${apply_mode}
- verified:
  - host access status returns a machine capability matrix with settings deeplinks
  - audit mode verifies post-probe state for every tracked service
  - apply flow is exercised when a missing service exists; otherwise steady-state readiness is confirmed
EOF

printf '%s\n' "${output_dir}"
