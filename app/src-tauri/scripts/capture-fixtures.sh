#!/usr/bin/env bash
# Capture real `container` CLI output into fixture files for offline parser tests.
#
#   scripts/capture-fixtures.sh <output-dir>
#
# Creates a temporary running container plus a volume and a network (all named
# `cgui-live-fx-<pid>-*`), captures each command's output to
# <output-dir>/<name>.json (container --version -> version.txt), then
# removes everything it created. Home-directory paths, hostnames and the
# username are redacted. Requires `container` on PATH with services running.
set -euo pipefail

out="${1:?usage: capture-fixtures.sh <output-dir>}"
mkdir -p "$out"
tag="cgui-live-fx-$$"
ctr="$tag-ctr"; vol="$tag-vol"; net="$tag-net"

cleanup() {
  container delete --force "$ctr" >/dev/null 2>&1 || true
  container volume delete "$vol" >/dev/null 2>&1 || true
  container network delete "$net" >/dev/null 2>&1 || true
}
trap cleanup EXIT

container network create "$net" >/dev/null
container volume create "$vol" >/dev/null
container run -d --name "$ctr" --network "$net" -v "$vol:/data" \
  -p 127.0.0.1:18080:80 -e CGUI_FIXTURE=1 alpine:latest sleep 300 >/dev/null
sleep 3

host="$(hostname -s)"
user="$(id -un)"
redact() {
  sed -E -e 's#/Users/[^/"\\ ]+#/Users/user#g' \
         -e "s#${host}(\\.local)?#host#g" \
         -e "s#${user}#user#g"
}
cap() { # <file> <cmd...>
  local f="$1"; shift
  if ! "$@" 2>/dev/null | redact >"$out/$f"; then echo "warn: $* failed" >&2; fi
}

# Drop the operator's own workloads/locally built images: keep only the
# builder, our cgui-live-* resources, and library/ + apple/ images.
filter_json() { # <kind>; reads stdin
  python3 -c '
import json, sys
kind = sys.argv[1]
d = json.load(sys.stdin)
if kind == "ls":
    d = [c for c in d if c["id"] == "buildkit" or c["id"].startswith("cgui-live-")]
elif kind == "images":
    d = [i for i in d if "/library/" in i["configuration"]["name"] or "/apple/" in i["configuration"]["name"]]
print(json.dumps(d))
' "$1"
}

cap version.txt container --version
cap system-status.json container system status --format json
container ls --all --format json 2>/dev/null | filter_json ls | redact >"$out/ls.json"
cap stats.json container stats --no-stream --format json
container image ls --format json 2>/dev/null | filter_json images | redact >"$out/image-ls.json"
cap volume-ls.json container volume ls --format json
cap network-ls.json container network ls --format json
cap system-df.json container system df --format json
cap builder-status.json container builder status --format json
cap machine-list.json container machine list --format json
cap registry-list.json container registry list --format json
echo "captured into $out"
