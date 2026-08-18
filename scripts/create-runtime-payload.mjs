import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'

const [archivePath, version, commit, channel, repository] = process.argv.slice(2)
if (![archivePath, version, commit, channel, repository].every(Boolean)) {
  throw new Error('usage: node create-runtime-payload.mjs <archive> <version> <commit> <channel> <owner/repo>')
}

const bytes = await readFile(archivePath)
const sha256 = createHash('sha256').update(bytes).digest('hex')
const file = path.basename(archivePath)
if (!['stable', 'beta'].includes(channel)) throw new Error(`unsupported channel: ${channel}`)
const tag = `runtime-${channel}`
const payload = {
  version,
  upstreamCommit: commit,
  archiveUrl: `https://github.com/${repository}/releases/download/${tag}/${file}`,
  sha256,
  desktopMinVersion: '0.1.0',
  desktopMaxVersion: '0.1.999',
  releaseNotes: `DeepSeek Harness runtime ${version} (${commit.slice(0, 12)})`,
}
await writeFile(`${archivePath}.payload.json`, `${JSON.stringify(payload, null, 2)}\n`)
