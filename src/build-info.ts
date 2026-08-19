import packageInfo from '../package.json'
import runtimeInfo from '../runtime/runtime-manifest.json'

export const DESKTOP_VERSION = packageInfo.version
export const RUNTIME_VERSION = runtimeInfo.runtimeVersion
export const UPSTREAM_COMMIT = runtimeInfo.upstreamCommit
