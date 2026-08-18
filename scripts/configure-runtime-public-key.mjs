import { createPrivateKey, createPublicKey } from 'node:crypto'
import { writeFile } from 'node:fs/promises'

const privateKeyPem = process.env.DSH_RUNTIME_SIGNING_PRIVATE_KEY
if (!privateKeyPem) throw new Error('DSH_RUNTIME_SIGNING_PRIVATE_KEY is required')

const publicDer = createPublicKey(createPrivateKey(privateKeyPem)).export({
  type: 'spki',
  format: 'der',
})
const ed25519SpkiPrefix = '302a300506032b6570032100'
if (publicDer.length !== 44 || publicDer.subarray(0, 12).toString('hex') !== ed25519SpkiPrefix) {
  throw new Error('DSH_RUNTIME_SIGNING_PRIVATE_KEY must be an Ed25519 private key')
}

const rawPublicKey = publicDer.subarray(publicDer.length - 32)
await writeFile('runtime/public-key.txt', `${rawPublicKey.toString('base64')}\n`)
