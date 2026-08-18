import { useCallback, useEffect, useMemo, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import {
  Activity,
  ArrowUpCircle,
  Check,
  CircleAlert,
  ExternalLink,
  FileArchive,
  Gauge,
  KeyRound,
  Languages,
  LoaderCircle,
  Play,
  RefreshCw,
  RotateCcw,
  Settings,
  ShieldCheck,
  Square,
  TerminalSquare,
} from 'lucide-react'
import { desktopApi } from './api'
import { translate, type Language, type MessageKey } from './i18n'
import type { DesktopPreferences, RuntimeState, RuntimeStatus, UpdateStatus } from './types'

type View = 'workbench' | 'updates' | 'settings' | 'diagnostics'

const defaultPreferences: DesktopPreferences = {
  channel: 'stable',
  language: navigator.language.startsWith('zh') ? 'zh-CN' : 'en',
  checkOnLaunch: true,
}

const initialRuntime: RuntimeStatus = {
  state: 'stopped',
  version: '0.1.0-rc.5',
  upstreamCommit: '47f9438',
  url: null,
  pid: null,
  lastError: null,
  rollbackAvailable: false,
}

function loadPreferences(): DesktopPreferences {
  const value = localStorage.getItem('desktop-preferences')
  if (value === null) return defaultPreferences
  try {
    return { ...defaultPreferences, ...JSON.parse(value) as Partial<DesktopPreferences> }
  } catch {
    return defaultPreferences
  }
}

function stateTone(state: RuntimeState): string {
  if (state === 'running') return 'positive'
  if (state === 'failed') return 'negative'
  if (state === 'starting' || state === 'stopping') return 'busy'
  return 'neutral'
}

function App() {
  const [view, setView] = useState<View>('workbench')
  const [runtime, setRuntime] = useState<RuntimeStatus>(initialRuntime)
  const [preferences, setPreferences] = useState(loadPreferences)
  const [updates, setUpdates] = useState<UpdateStatus[]>([])
  const [checking, setChecking] = useState(false)
  const [busyRuntime, setBusyRuntime] = useState(false)
  const [apiKey, setApiKey] = useState('')
  const [notice, setNotice] = useState<string | null>(null)
  const t = useCallback((key: MessageKey) => translate(preferences.language, key), [preferences.language])

  const checkUpdates = useCallback(async () => {
    setChecking(true)
    try {
      const result = await Promise.all([
        desktopApi.checkAppUpdate(preferences.channel),
        desktopApi.checkRuntimeUpdate(preferences.channel),
      ])
      setUpdates(result)
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error))
    } finally {
      setChecking(false)
    }
  }, [preferences.channel])

  useEffect(() => {
    void desktopApi.runtimeStatus().then(setRuntime).catch(error => {
      setRuntime(current => ({ ...current, state: 'failed', lastError: String(error) }))
    })
    void desktopApi.secureGet('deepseek-api-key').then(value => setApiKey(value ?? ''))
    if (preferences.checkOnLaunch) void checkUpdates()
  }, [])

  useEffect(() => {
    localStorage.setItem('desktop-preferences', JSON.stringify(preferences))
    document.documentElement.lang = preferences.language
  }, [preferences])

  const runRuntimeAction = useCallback(async (action: 'start' | 'stop' | 'restart') => {
    setBusyRuntime(true)
    setNotice(null)
    try {
      const result = action === 'start'
        ? await desktopApi.runtimeStart()
        : action === 'stop'
          ? await desktopApi.runtimeStop()
          : await desktopApi.runtimeRestart()
      setRuntime(result)
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error))
      void desktopApi.runtimeStatus().then(setRuntime)
    } finally {
      setBusyRuntime(false)
    }
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<string>('desktop-menu', event => {
      if (event.payload === 'settings') setView('settings')
      if (event.payload === 'updates') {
        setView('updates')
        void checkUpdates()
      }
      if (event.payload === 'runtime-start') void runRuntimeAction('start')
      if (event.payload === 'runtime-restart') void runRuntimeAction('restart')
    }).then(dispose => {
      if (disposed) dispose()
      else unlisten = dispose
    }).catch(() => {})
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [checkUpdates, runRuntimeAction])

  const saveKey = async () => {
    await desktopApi.secureSet('deepseek-api-key', apiKey)
    setNotice(t('saved'))
  }

  const deleteKey = async () => {
    await desktopApi.secureDelete('deepseek-api-key')
    setApiKey('')
    setNotice(t('removed'))
  }

  const exportDiagnostics = async () => {
    const result = await desktopApi.exportDiagnostics()
    setNotice(`${t('exported')}: ${result.path}`)
  }

  const rollbackRuntime = async () => {
    setBusyRuntime(true)
    try {
      setRuntime(await desktopApi.runtimeRollback())
      setNotice(t('rollbackComplete'))
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error))
    } finally {
      setBusyRuntime(false)
    }
  }

  const clearLogs = async () => {
    try {
      await desktopApi.clearLogs()
      setNotice(t('logsCleared'))
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error))
    }
  }

  const installUpdate = async (update: UpdateStatus) => {
    setUpdates(current => current.map(item => item.component === update.component
      ? { ...item, phase: 'installing', progress: 0 }
      : item))
    try {
      const installed = update.component === 'desktop'
        ? await desktopApi.installAppUpdate(preferences.channel)
        : await desktopApi.installRuntimeUpdate(update.availableVersion as string, preferences.channel)
      setUpdates(current => current.map(item => item.component === installed.component ? installed : item))
      if (update.component === 'runtime') void desktopApi.runtimeStatus().then(setRuntime)
    } catch (error) {
      setUpdates(current => current.map(item => item.component === update.component
        ? { ...item, phase: 'failed', errorCode: 'INSTALL_FAILED' }
        : item))
      setNotice(error instanceof Error ? error.message : String(error))
    }
  }

  const navItems = useMemo(() => [
    { id: 'workbench' as const, label: t('workbench'), icon: TerminalSquare },
    { id: 'updates' as const, label: t('updates'), icon: ArrowUpCircle },
    { id: 'settings' as const, label: t('settings'), icon: Settings },
    { id: 'diagnostics' as const, label: t('diagnostics'), icon: Activity },
  ], [t])

  return (
    <main className="app-shell">
      <aside className="sidebar" data-tauri-drag-region>
        <div className="traffic-light-space" data-tauri-drag-region />
        <div className="brand" data-tauri-drag-region>
          <span className="brand-mark">DS</span>
          <span>{t('appName')}</span>
        </div>
        <nav className="navigation" aria-label="Primary">
          {navItems.map(item => {
            const Icon = item.icon
            return (
              <button
                className={view === item.id ? 'nav-item active' : 'nav-item'}
                key={item.id}
                onClick={() => setView(item.id)}
              >
                <Icon size={17} strokeWidth={1.8} />
                <span>{item.label}</span>
              </button>
            )
          })}
        </nav>
        <div className="runtime-rail-status">
          <span className={`status-dot ${stateTone(runtime.state)}`} />
          <div>
            <strong>{t(runtime.state as MessageKey)}</strong>
            <span>{runtime.version}</span>
          </div>
        </div>
      </aside>

      <section className="content">
        <header className="toolbar" data-tauri-drag-region>
          <div>
            <h1>{t(view)}</h1>
            {view === 'workbench' && <span className="toolbar-subtitle">{t('runtime')} {runtime.version}</span>}
          </div>
          <div className="toolbar-actions">
            {view === 'workbench' && runtime.state !== 'running' && (
              <button className="primary icon-text" disabled={busyRuntime} onClick={() => void runRuntimeAction('start')}>
                {busyRuntime ? <LoaderCircle className="spin" size={16} /> : <Play size={16} fill="currentColor" />}
                {t('start')}
              </button>
            )}
            {view === 'workbench' && runtime.state === 'running' && (
              <>
                <button className="icon-button" title={t('restart')} disabled={busyRuntime} onClick={() => void runRuntimeAction('restart')}>
                  <RotateCcw size={17} />
                </button>
                <button className="icon-button" title={t('stop')} disabled={busyRuntime} onClick={() => void runRuntimeAction('stop')}>
                  <Square size={15} fill="currentColor" />
                </button>
              </>
            )}
            {view === 'updates' && (
              <button className="primary icon-text" disabled={checking} onClick={() => void checkUpdates()}>
                <RefreshCw className={checking ? 'spin' : ''} size={16} />
                {t('checkAll')}
              </button>
            )}
          </div>
        </header>

        {notice !== null && (
          <div className="notice" role="status">
            <CircleAlert size={16} />
            <span>{notice}</span>
            <button className="dismiss" onClick={() => setNotice(null)} aria-label="Dismiss">×</button>
          </div>
        )}

        {view === 'workbench' && (
          <section className="workbench">
            {runtime.state === 'running' && runtime.url !== null ? (
              <iframe className="harness-frame" title="DeepSeek Harness" src={runtime.url} allow="clipboard-read; clipboard-write" />
            ) : (
              <div className="empty-state">
                <div className={`runtime-glyph ${stateTone(runtime.state)}`}>
                  {runtime.state === 'starting' ? <LoaderCircle className="spin" /> : <Gauge />}
                </div>
                <h2>{t('runtimeUnavailable')}</h2>
                {runtime.lastError !== null && <code>{runtime.lastError}</code>}
                <button className="primary icon-text" disabled={busyRuntime} onClick={() => void runRuntimeAction('start')}>
                  <Play size={16} fill="currentColor" />
                  {t('start')}
                </button>
              </div>
            )}
          </section>
        )}

        {view === 'updates' && (
          <section className="page-section updates-list">
            {(['desktop', 'runtime'] as const).map(component => {
              const update = updates.find(item => item.component === component)
              const available = update?.phase === 'available' && update.availableVersion !== null
              return (
                <article className="update-row" key={component}>
                  <div className="component-icon">
                    {component === 'desktop' ? <ShieldCheck size={22} /> : <Gauge size={22} />}
                  </div>
                  <div className="update-copy">
                    <div className="update-title-line">
                      <h2>{component === 'desktop' ? t('desktopApp') : t('runtime')}</h2>
                      <span className={available ? 'version-pill available' : 'version-pill'}>
                        {update?.availableVersion ?? update?.currentVersion ?? runtime.version}
                      </span>
                    </div>
                    <p>{checking ? t('checking') : available ? t('available') : t('current')}</p>
                    {update?.releaseNotes && <p className="release-notes">{update.releaseNotes}</p>}
                    {update?.errorCode === 'UPDATE_SOURCE_NOT_CONFIGURED' && <p className="secondary-text">{t('notConfigured')}</p>}
                    {(update?.phase === 'downloading' || update?.phase === 'installing') && (
                      <progress aria-label={t('progress')} value={update.progress} max="100" />
                    )}
                  </div>
                  <button
                    className="secondary icon-text"
                    disabled={!available}
                    onClick={() => { if (update) void installUpdate(update) }}
                  >
                    <ArrowUpCircle size={16} />
                    {t('install')}
                  </button>
                </article>
              )
            })}
          </section>
        )}

        {view === 'settings' && (
          <section className="page-section settings-form">
            <div className="setting-row">
              <div className="setting-label">
                <ArrowUpCircle size={18} />
                <span>{t('updateChannel')}</span>
              </div>
              <div className="segmented-control" role="group" aria-label={t('updateChannel')}>
                {(['stable', 'beta'] as const).map(channel => (
                  <button
                    className={preferences.channel === channel ? 'selected' : ''}
                    key={channel}
                    onClick={() => setPreferences(current => ({ ...current, channel }))}
                  >
                    {t(channel)}
                  </button>
                ))}
              </div>
            </div>
            <div className="setting-row">
              <div className="setting-label">
                <RefreshCw size={18} />
                <span>{t('checkOnLaunch')}</span>
              </div>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={preferences.checkOnLaunch}
                  onChange={event => setPreferences(current => ({ ...current, checkOnLaunch: event.target.checked }))}
                />
                <span />
              </label>
            </div>
            <div className="setting-row">
              <div className="setting-label">
                <Languages size={18} />
                <span>{t('language')}</span>
              </div>
              <select
                value={preferences.language}
                onChange={event => setPreferences(current => ({ ...current, language: event.target.value as Language }))}
              >
                <option value="zh-CN">简体中文</option>
                <option value="en">English</option>
              </select>
            </div>
            <div className="setting-row key-setting">
              <div className="setting-label">
                <KeyRound size={18} />
                <span>{t('apiKey')}</span>
              </div>
              <div className="key-field">
                <input
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={apiKey}
                  onChange={event => setApiKey(event.target.value)}
                />
                <button className="secondary" disabled={apiKey.length === 0} onClick={() => void saveKey()}>{t('save')}</button>
                <button className="text-button" disabled={apiKey.length === 0} onClick={() => void deleteKey()}>{t('clear')}</button>
              </div>
            </div>
          </section>
        )}

        {view === 'diagnostics' && (
          <section className="page-section diagnostics-grid">
            <dl>
              <div><dt>{t('version')}</dt><dd>{runtime.version}</dd></div>
              <div><dt>{t('upstream')}</dt><dd><code>{runtime.upstreamCommit.slice(0, 12)}</code></dd></div>
              <div><dt>{t('process')}</dt><dd>{runtime.pid ?? '—'}</dd></div>
              <div><dt>{t('rollback')}</dt><dd>{runtime.rollbackAvailable ? <Check size={16} /> : '—'}</dd></div>
            </dl>
            <button className="secondary icon-text export-button" onClick={() => void exportDiagnostics()}>
              <FileArchive size={17} />
              {t('exportDiagnostics')}
            </button>
            <div className="diagnostic-actions">
              <button className="secondary" disabled={!runtime.rollbackAvailable || busyRuntime} onClick={() => void rollbackRuntime()}>
                {t('rollbackRuntime')}
              </button>
              <button className="secondary" onClick={() => void clearLogs()}>{t('clearLogs')}</button>
            </div>
            {runtime.url !== null && (
              <a className="runtime-link" href={runtime.url} target="_blank" rel="noreferrer">
                {runtime.url}<ExternalLink size={14} />
              </a>
            )}
          </section>
        )}
      </section>
    </main>
  )
}

export default App
