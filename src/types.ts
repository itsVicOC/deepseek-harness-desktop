export type RuntimeState = 'stopped' | 'starting' | 'running' | 'stopping' | 'failed'
export type UpdateChannel = 'stable' | 'beta'
export type UpdatePhase = 'idle' | 'checking' | 'available' | 'downloading' | 'installing' | 'handedoff' | 'current' | 'failed'

export interface RuntimeStatus {
  state: RuntimeState
  version: string
  upstreamCommit: string
  url: string | null
  pid: number | null
  lastError: string | null
  rollbackAvailable: boolean
}

export interface UpdateStatus {
  component: 'desktop' | 'runtime'
  currentVersion: string
  availableVersion: string | null
  channel: UpdateChannel
  phase: UpdatePhase
  progress: number
  requiresRestart: boolean
  errorCode: string | null
  rollbackAvailable: boolean
  releaseNotes: string | null
}

export interface DiagnosticsResult {
  path: string
  createdAt: string
}

export interface DesktopPreferences {
  channel: UpdateChannel
  language: 'zh-CN' | 'en'
  checkOnLaunch: boolean
}
