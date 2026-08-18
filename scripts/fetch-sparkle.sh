#!/bin/sh
set -eu

SPARKLE_VERSION=${SPARKLE_VERSION:-2.7.1}
: "${SPARKLE_ARCHIVE_SHA256:?SPARKLE_ARCHIVE_SHA256 is required}"

WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/sparkle.XXXXXX")
trap 'rm -rf "$WORK_DIR"' EXIT INT TERM
ARCHIVE="$WORK_DIR/Sparkle.tar.xz"
URL="https://github.com/sparkle-project/Sparkle/releases/download/${SPARKLE_VERSION}/Sparkle-${SPARKLE_VERSION}.tar.xz"

curl -fsSL "$URL" -o "$ARCHIVE"
ACTUAL=$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')
if [ "$ACTUAL" != "$SPARKLE_ARCHIVE_SHA256" ]; then
  echo "Sparkle archive checksum mismatch" >&2
  exit 1
fi

tar -C "$WORK_DIR" -xf "$ARCHIVE"
mkdir -p src-tauri/native/frameworks
ditto "$WORK_DIR/Sparkle.framework" src-tauri/native/frameworks/Sparkle.framework
