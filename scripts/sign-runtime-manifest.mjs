import { createPrivateKey, sign } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'

const [payloadPath, outputPath] = process.argv.slice(2)
if (!payloadPath || !outputPath) {
  throw new Error('usage: node sign-runtime-manifest.mjs <payload.json> <signed-manifest.json>')
}

const privateKeyPem = process.env.DSH_RUNTIME_SIGNING_PRIVATE_KEY
if (!privateKeyPem) throw new Error('DSH_RUNTIME_SIGNING_PRIVATE_KEY is required')

const payload = JSON.parse(await readFile(payloadPath, 'utf8'))
const canonical = Buffer.from(JSON.stringify(payload))
const signature = sign(null, canonical, createPrivateKey(privateKeyPem)).toString('base64')
const manifest = { schemaVersion: 1, payload, signature }
await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`)
