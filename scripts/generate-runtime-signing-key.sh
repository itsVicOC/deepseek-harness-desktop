#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <private-key-output-path>" >&2
  exit 2
fi

OUTPUT=$1
if [ -e "$OUTPUT" ]; then
  echo "refusing to overwrite existing key: $OUTPUT" >&2
  exit 1
fi

umask 077
if command -v openssl >/dev/null 2>&1 && openssl list -public-key-algorithms 2>/dev/null | grep -qi ed25519; then
  openssl genpkey -algorithm Ed25519 -out "$OUTPUT"
elif command -v node >/dev/null 2>&1; then
  node --input-type=module -e '
    import { generateKeyPairSync } from "node:crypto"
    import { writeFileSync } from "node:fs"
    const output = process.argv[1]
    const { privateKey } = generateKeyPairSync("ed25519")
    writeFileSync(output, privateKey.export({ type: "pkcs8", format: "pem" }), { mode: 0o600 })
  ' "$OUTPUT"
else
  echo "openssl with Ed25519 support or Node.js is required" >&2
  exit 1
fi
echo "Created an Ed25519 private key at $OUTPUT" >&2
echo "Store it in GitHub Actions as DSH_RUNTIME_SIGNING_PRIVATE_KEY; never commit this file." >&2
