#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <runtime-root> <expected-version> <expected-commit>" >&2
  exit 2
fi

RUNTIME_ROOT=$1
EXPECTED_VERSION=$2
EXPECTED_COMMIT=$3

test -x "$RUNTIME_ROOT/bin/dsh"
test -x "$RUNTIME_ROOT/bin/node"
test -f "$RUNTIME_ROOT/runtime.json"

node -e '
const fs = require("node:fs")
const [manifestPath, expectedVersion, expectedCommit] = process.argv.slice(1)
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"))
if (manifest.runtimeVersion !== expectedVersion) throw new Error(`runtime version mismatch: ${manifest.runtimeVersion}`)
if (manifest.upstreamCommit !== expectedCommit) throw new Error(`upstream commit mismatch: ${manifest.upstreamCommit}`)
' "$RUNTIME_ROOT/runtime.json" "$EXPECTED_VERSION" "$EXPECTED_COMMIT"

ACTUAL_VERSION=$("$RUNTIME_ROOT/bin/dsh" --version)
if [ "$ACTUAL_VERSION" != "$EXPECTED_VERSION" ]; then
  echo "runtime launcher version mismatch: expected $EXPECTED_VERSION, got $ACTUAL_VERSION" >&2
  exit 1
fi
