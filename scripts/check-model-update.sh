#!/bin/sh
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$SCRIPT_DIR/ota-lib.sh"

SMARTHOME_DIR="${SMARTHOME_DIR:-/etc/smarthome}"
AI_INFO_FILE="${AI_INFO_FILE:-$SMARTHOME_DIR/ota_ai_info.json}"
HC_CONFIG_FILE="${HC_CONFIG_FILE:-$SMARTHOME_DIR/hc-config.json}"
MAC_FILE="${MAC_FILE:-$SMARTHOME_DIR/mac_addr.txt}"
WORK_DIR="${WORK_DIR:-$SMARTHOME_DIR/ota-ai-work}"
RESTART_AIBOX="${RESTART_AIBOX:-0}"
DEFAULT_DETECT_PATH="${DEFAULT_DETECT_PATH:-$SMARTHOME_DIR/detect_model_v2.lum}"
DEFAULT_VERIFY_PATH="${DEFAULT_VERIFY_PATH:-$SMARTHOME_DIR/verify_model_v2.lum}"

if [ "$(id -u)" -ne 0 ]; then
  echo "This script must run as root" >&2
  exit 1
fi

mkdir -p "$WORK_DIR"

MAC="${MAC:-$(cat "$MAC_FILE" | tr -d '[:space:]')}"
HC_DIR="$(mac_to_dir "$MAC")"

if [ -n "${OTA_REPO_DIR:-}" ]; then
  MANIFEST_SRC="$OTA_REPO_DIR/hc/$HC_DIR/manifest.json"
  CONFIG_BASE="$OTA_REPO_DIR/hc/$HC_DIR"
else
  OTA_BASE_URL="${OTA_BASE_URL:?OTA_BASE_URL or OTA_REPO_DIR is required}"
  MANIFEST_SRC="$OTA_BASE_URL/hc/$HC_DIR/manifest.json"
  CONFIG_BASE="$OTA_BASE_URL/hc/$HC_DIR"
fi

MANIFEST_TMP="$WORK_DIR/manifest.json.tmp"
fetch_file "$MANIFEST_SRC" "$MANIFEST_TMP"

local_detect="$(json_get_default detect_version 0 "$AI_INFO_FILE")"
local_verify="$(json_get_default verify_version 0 "$AI_INFO_FILE")"
local_config="$(json_get_default config_version 0 "$AI_INFO_FILE")"

remote_detect="$(json_get_default detect.version "$local_detect" "$MANIFEST_TMP")"
remote_verify="$(json_get_default verify.version "$local_verify" "$MANIFEST_TMP")"
remote_config="$(json_get_default config.version "$local_config" "$MANIFEST_TMP")"

changed=0

update_model_if_needed() {
  name="$1"
  remote_version="$2"
  local_version="$3"
  default_target="$4"

  if [ "$remote_version" -le "$local_version" ]; then
    log "$name version $remote_version <= local $local_version, skip"
    return 0
  fi

  url="$(json_get "$name.url" "$MANIFEST_TMP")"
  target="$(json_get_default "$name.path" "$default_target" "$MANIFEST_TMP")"
  expected_sha="$(json_get "$name.sha256" "$MANIFEST_TMP")"
  tmp="$WORK_DIR/$name.tmp"

  log "Updating $name model: $local_version -> $remote_version at $target"
  fetch_file "$url" "$tmp"
  verify_sha256 "$tmp" "$expected_sha"
  cp "$tmp" "$target"
  changed=1
}

rollback_to_default_models() {
  log "Rolling back aibox model paths to default models"
  set_aibox_env_values \
    "$HC_CONFIG_FILE" \
    "$WORK_DIR/hc-config.rollback.tmp" \
    "MODEL_PATH=$DEFAULT_DETECT_PATH" \
    "VERIFY_MODEL_PATH=$DEFAULT_VERIFY_PATH"
}

restart_and_check_aibox() {
  log "Restarting aibox"
  pkill -f '/usr/bin/aibox' || true
  for _ in $(seq 1 30); do
    if pgrep -f '/usr/bin/aibox' >/dev/null 2>&1; then
      log "aibox is running"
      return 0
    fi
    sleep 1
  done
  log "aibox health check failed"
  return 1
}

update_model_if_needed detect "$remote_detect" "$local_detect" "$DEFAULT_DETECT_PATH"
update_model_if_needed verify "$remote_verify" "$local_verify" "$DEFAULT_VERIFY_PATH"

if [ "$remote_config" -gt "$local_config" ]; then
  config_file="$(json_get config.file "$MANIFEST_TMP")"
  expected_config_sha="$(json_get config.sha256 "$MANIFEST_TMP")"
  config_src="$CONFIG_BASE/$config_file"
  config_tmp="$WORK_DIR/config.json.tmp"
  merged_tmp="$WORK_DIR/hc-config.json.tmp"

  log "Updating config: $local_config -> $remote_config"
  fetch_file "$config_src" "$config_tmp"
  verify_sha256 "$config_tmp" "$expected_config_sha"
  apply_aibox_env_config "$config_tmp" "$HC_CONFIG_FILE" "$merged_tmp"
  cp "$merged_tmp" "$HC_CONFIG_FILE"
  changed=1
else
  log "config version $remote_config <= local $local_config, skip"
fi

if [ "$changed" -eq 1 ] && [ "$RESTART_AIBOX" = "1" ]; then
  if ! restart_and_check_aibox; then
    rollback_to_default_models
    restart_and_check_aibox || true
    update_ai_info "$AI_INFO_FILE" "$local_detect" "$local_verify" "$local_config" "failed"
    exit 1
  fi
fi

if [ "$changed" -eq 1 ]; then
  update_ai_info "$AI_INFO_FILE" "$remote_detect" "$remote_verify" "$remote_config" "success"
  log "OTA AI update success"
else
  log "No OTA AI update needed"
fi
