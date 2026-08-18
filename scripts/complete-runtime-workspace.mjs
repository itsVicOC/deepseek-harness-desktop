import { cp, lstat, mkdir, readdir, realpath, symlink } from 'node:fs/promises'
import path from 'node:path'

const [deployRootArg, upstreamRootArg] = process.argv.slice(2)
if (!deployRootArg || !upstreamRootArg) {
  throw new Error('usage: node complete-runtime-workspace.mjs <deploy-root> <upstream-root>')
}

const deployRoot = path.resolve(deployRootArg)
const upstreamRoot = await realpath(path.resolve(upstreamRootArg))
const workspaceScope = path.join(upstreamRoot, 'node_modules/.pnpm/node_modules/@deepseek-ai')
const targetScope = path.join(deployRoot, 'node_modules/@deepseek-ai')
await mkdir(targetScope, { recursive: true })

async function exists(target) {
  try {
    await lstat(target)
    return true
  } catch (error) {
    if (error.code === 'ENOENT') return false
    throw error
  }
}

async function linkInstalled(source, target) {
  try {
    await realpath(source)
  } catch (error) {
    if (error.code === 'ENOENT') return
    throw error
  }
  if (await exists(target)) return
  await mkdir(path.dirname(target), { recursive: true })
  await symlink(path.relative(path.dirname(target), source), target)
}

const installedRoot = path.join(deployRoot, 'node_modules/.pnpm/node_modules')
for (const name of (await readdir(installedRoot)).sort()) {
  const source = path.join(installedRoot, name)
  if (name.startsWith('@')) {
    for (const scopedName of (await readdir(source)).sort()) {
      await linkInstalled(
        path.join(source, scopedName),
        path.join(deployRoot, 'node_modules', name, scopedName),
      )
    }
  } else {
    await linkInstalled(source, path.join(deployRoot, 'node_modules', name))
  }
}

const workspaceTargets = [
  path.join(deployRoot, 'node_modules', '@deepseek-ai'),
  path.join(deployRoot, 'node_modules/.pnpm/node_modules', '@deepseek-ai'),
]

for (const targetScope of workspaceTargets) {
  await mkdir(targetScope, { recursive: true })
}

for (const name of (await readdir(workspaceScope)).sort()) {
  const source = await realpath(path.join(workspaceScope, name))
  if (source !== upstreamRoot && !source.startsWith(`${upstreamRoot}${path.sep}`)) continue
  for (const targetScope of workspaceTargets) {
    const target = path.join(targetScope, name)
    if (await exists(target)) {
      const targetInfo = await lstat(target)
      if (!targetInfo.isSymbolicLink()) continue
      await cp(source, target, {
        recursive: true,
        dereference: true,
        force: true,
        verbatimSymlinks: false,
      })
      continue
    }
    await cp(source, target, {
      recursive: true,
      filter: current => path.basename(current) !== 'node_modules' && path.basename(current) !== '.git',
    })
  }
}
