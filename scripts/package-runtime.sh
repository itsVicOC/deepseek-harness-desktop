#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <deepseek-harness-checkout> <output-directory>" >&2
  exit 2
fi

UPSTREAM_DIR=$(cd "$1" && pwd)
OUTPUT_DIR=$2
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/dsh-runtime.XXXXXX")
trap 'rm -rf "$WORK_DIR"' EXIT INT TERM

EXPECTED_COMMIT=$(node -p "require('${UPSTREAM_DIR}/package.json').version" >/dev/null && git -C "$UPSTREAM_DIR" rev-parse HEAD)
RUNTIME_VERSION=$(node -p "require('${UPSTREAM_DIR}/apps/cli/package.json').version")
NODE_BIN=$(command -v node)

# Release CI installs a checksum-pinned pnpm version. Keep using that verified
# binary instead of letting pnpm fetch the upstream packageManager version.
export CI=true
export pnpm_config_pm_on_fail=ignore
pnpm --pm-on-fail=ignore --dir "$UPSTREAM_DIR" install --frozen-lockfile
pnpm --pm-on-fail=ignore --dir "$UPSTREAM_DIR" run build
pnpm --pm-on-fail=ignore --dir "$UPSTREAM_DIR" --filter @deepseek-ai/dsh deploy --legacy "$WORK_DIR/dsh"

# rc.5 loads workspace peers from profile YAML rather than the CLI dependency
# graph. Flatten the built internal packages without their node_modules so the
# deployed external dependency store remains authoritative.
node "$SCRIPT_DIR/complete-runtime-workspace.mjs" "$WORK_DIR/dsh" "$UPSTREAM_DIR"

mkdir -p "$WORK_DIR/package/bin"
cp "$NODE_BIN" "$WORK_DIR/package/bin/node"
cp -R "$WORK_DIR/dsh" "$WORK_DIR/package/dsh"

cat > "$WORK_DIR/package/bin/dsh" <<'LAUNCHER'
#!/bin/sh
RUNTIME_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$RUNTIME_ROOT/bin/node" "$RUNTIME_ROOT/dsh/lib/bin.js" "$@"
LAUNCHER
chmod 755 "$WORK_DIR/package/bin/dsh" "$WORK_DIR/package/bin/node"

node -e '
const fs = require("node:fs")
const [target, version, commit] = process.argv.slice(1)
fs.writeFileSync(target, `${JSON.stringify({ runtimeVersion: version, upstreamCommit: commit }, null, 2)}\n`)
' "$WORK_DIR/package/runtime.json" "$RUNTIME_VERSION" "$EXPECTED_COMMIT"

mkdir -p "$OUTPUT_DIR"
rm -rf "$OUTPUT_DIR/current"
tar -C "$WORK_DIR/package" -czf "$OUTPUT_DIR/dsh-runtime-${RUNTIME_VERSION}-darwin-arm64.tar.gz" .
cp -R "$WORK_DIR/package/." "$OUTPUT_DIR/current"

echo "$RUNTIME_VERSION" > "$OUTPUT_DIR/runtime-version.txt"
echo "$EXPECTED_COMMIT" > "$OUTPUT_DIR/upstream-commit.txt"
