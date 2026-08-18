import { lstat, readdir, realpath, unlink } from 'node:fs/promises'
import path from 'node:path'

const [rootArg] = process.argv.slice(2)
if (!rootArg) throw new Error('usage: node prune-runtime-links.mjs <runtime-root>')

async function walk(root) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const target = path.join(root, entry.name)
    if (entry.isSymbolicLink()) {
      try {
        await realpath(target)
      } catch (error) {
        if (error.code === 'ENOENT') await unlink(target)
        else throw error
      }
      continue
    }
    if (entry.isDirectory()) await walk(target)
  }
}

await walk(path.resolve(rootArg))
