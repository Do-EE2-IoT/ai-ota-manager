#!/bin/sh
set -eu

log() {
  printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*"
}

mac_to_dir() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | tr ':' '-'
}

json_get() {
  python3 - "$1" "$2" <<'PY'
import json
import sys

path = sys.argv[1].split(".")
with open(sys.argv[2], "r", encoding="utf-8") as f:
    data = json.load(f)
for key in path:
    data = data[key]
print(data)
PY
}

json_get_default() {
  python3 - "$1" "$2" "$3" <<'PY'
import json
import sys

path = sys.argv[1].split(".")
default = sys.argv[2]
with open(sys.argv[3], "r", encoding="utf-8") as f:
    data = json.load(f)
try:
    for key in path:
        data = data[key]
except Exception:
    print(default)
else:
    print(data)
PY
}

sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}

verify_sha256() {
  file="$1"
  expected="$2"
  actual="$(sha256_file "$file")"
  if [ "$actual" != "$expected" ]; then
    log "SHA256 mismatch for $file: expected=$expected actual=$actual"
    return 1
  fi
}

fetch_file() {
  src="$1"
  dst="$2"
  case "$src" in
    http://*|https://*)
      if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$src" -o "$dst"
      else
        wget -q "$src" -O "$dst"
      fi
      ;;
    *)
      cp "$src" "$dst"
      ;;
  esac
}

apply_aibox_env_config() {
  config_file="$1"
  target_file="$2"
  tmp_file="$3"

  python3 - "$config_file" "$target_file" "$tmp_file" <<'PY'
import json
import sys

config_path, target_path, tmp_path = sys.argv[1:4]
with open(config_path, "r", encoding="utf-8") as f:
    ota_config = json.load(f)
env = ota_config.get("env")
if not isinstance(env, list) or not all(isinstance(item, str) and "=" in item for item in env):
    raise SystemExit("config.env must be an array of KEY=VALUE strings")

with open(target_path, "r", encoding="utf-8") as f:
    hc_config = json.load(f)

hc_config.setdefault("bridge_component", {}).setdefault("aibox", {})["env"] = env

with open(tmp_path, "w", encoding="utf-8") as f:
    json.dump(hc_config, f, ensure_ascii=False, indent=2)
    f.write("\n")
PY
}

set_aibox_env_values() {
  target_file="$1"
  tmp_file="$2"
  shift 2

  python3 - "$target_file" "$tmp_file" "$@" <<'PY'
import json
import sys

target_path, tmp_path, *assignments = sys.argv[1:]
updates = dict(item.split("=", 1) for item in assignments)

with open(target_path, "r", encoding="utf-8") as f:
    hc_config = json.load(f)

env = hc_config.setdefault("bridge_component", {}).setdefault("aibox", {}).setdefault("env", [])
next_env = []
seen = set()
for item in env:
    if "=" not in item:
        next_env.append(item)
        continue
    key, value = item.split("=", 1)
    if key in updates:
        next_env.append(f"{key}={updates[key]}")
        seen.add(key)
    else:
        next_env.append(item)

for key, value in updates.items():
    if key not in seen:
        next_env.append(f"{key}={value}")

hc_config["bridge_component"]["aibox"]["env"] = next_env

with open(tmp_path, "w", encoding="utf-8") as f:
    json.dump(hc_config, f, ensure_ascii=False, indent=2)
    f.write("\n")
PY
  mv "$tmp_file" "$target_file"
}

update_ai_info() {
  info_file="$1"
  detect_version="$2"
  verify_version="$3"
  config_version="$4"
  status="$5"
  tmp_file="${info_file}.tmp"

  python3 - "$info_file" "$detect_version" "$verify_version" "$config_version" "$status" "$tmp_file" <<'PY'
import json
import os
import sys
from datetime import datetime, timezone

info_path, detect_v, verify_v, config_v, status, tmp_path = sys.argv[1:7]
data = {}
if os.path.exists(info_path):
    with open(info_path, "r", encoding="utf-8") as f:
        data = json.load(f)
data["detect_version"] = int(detect_v)
data["verify_version"] = int(verify_v)
data["config_version"] = int(config_v)
data["last_update"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
data["status"] = status
with open(tmp_path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
  mv "$tmp_file" "$info_file"
}
