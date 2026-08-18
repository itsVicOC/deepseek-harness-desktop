import { invoke } from '@tauri-apps/api/core'
import type { DiagnosticsResult, RuntimeStatus, UpdateChannel, UpdateStatus } from './types'

const isTauri = typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined
const developmentRuntimeUrl = import.meta.env.VITE_DSH_RUNTIME_URL ?? 'http://127.0.0.1:3081'

const developmentRuntime: RuntimeStatus = {
  state: 'stopped',
  version: '0.1.0-rc.5',
  upstreamCommit: '47f943859bef60e4160492346772ded9b24f765a',
  url: null,
  pid: null,
  lastError: null,
  rollbackAvailable: false,
}

function unavailableUpdate(component: 'desktop' | 'runtime', channel: UpdateChannel): UpdateStatus {
  return {
    component,
    currentVersion: component === 'desktop' ? '0.1.0' : developmentRuntime.version,
    availableVersion: null,
    channel,
    phase: 'current',
    progress: 0,
    requiresRestart: component === 'desktop',
    errorCode: 'UPDATE_SOURCE_NOT_CONFIGURED',
    rollbackAvailable: false,
    releaseNotes: null,
  }
}

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) throw new Error(`Tauri command ${name} is unavailable in browser preview`)
  return invoke<T>(name, args)
}

export const desktopApi = {
  async runtimeStatus(): Promise<RuntimeStatus> {
    return isTauri ? command('runtime_status') : { ...developmentRuntime }
  },

  async runtimeStart(): Promise<RuntimeStatus> {
    if (!isTauri) return { ...developmentRuntime, state: 'running', url: developmentRuntimeUrl, pid: 1024 }
    return command('runtime_start')
  },

  async runtimeStop(): Promise<RuntimeStatus> {
    if (!isTauri) return { ...developmentRuntime }
    return command('runtime_stop')
  },

  async runtimeRestart(): Promise<RuntimeStatus> {
    if (!isTauri) return { ...developmentRuntime, state: 'running', url: developmentRuntimeUrl, pid: 1025 }
    return command('runtime_restart')
  },

  async runtimeRollback(): Promise<RuntimeStatus> {
    if (!isTauri) return { ...developmentRuntime }
    return command('runtime_rollback')
  },

  async checkRuntimeUpdate(channel: UpdateChannel): Promise<UpdateStatus> {
    return isTauri ? command('runtime_update_check', { channel }) : unavailableUpdate('runtime', channel)
  },

  async installRuntimeUpdate(version: string, channel: UpdateChannel): Promise<UpdateStatus> {
    return command('runtime_update_install', { version, channel })
  },

  async checkAppUpdate(channel: UpdateChannel): Promise<UpdateStatus> {
    return isTauri ? command('app_update_check', { channel }) : unavailableUpdate('desktop', channel)
  },

  async installAppUpdate(channel: UpdateChannel): Promise<UpdateStatus> {
    return command('app_update_install', { channel })
  },

  async secureGet(key: string): Promise<string | null> {
    return isTauri ? command('secure_get', { key }) : sessionStorage.getItem(`secure:${key}`)
  },

  async secureSet(key: string, value: string): Promise<void> {
    if (isTauri) await command('secure_set', { key, value })
    else sessionStorage.setItem(`secure:${key}`, value)
  },

  async secureDelete(key: string): Promise<void> {
    if (isTauri) await command('secure_delete', { key })
    else sessionStorage.removeItem(`secure:${key}`)
  },

  async exportDiagnostics(): Promise<DiagnosticsResult> {
    if (isTauri) return command('diagnostics_export')
    return { path: '/tmp/deepseek-harness-diagnostics.zip', createdAt: new Date().toISOString() }
  },

  async clearLogs(): Promise<void> {
    if (isTauri) await command('logs_clear')
  },
}
