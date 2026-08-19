import type { UpdatePhase, UpdateStatus } from './types'

const allowedTransitions: Record<UpdatePhase, readonly UpdatePhase[]> = {
  idle: ['checking'],
  checking: ['available', 'current', 'failed'],
  available: ['checking', 'downloading'],
  downloading: ['installing', 'failed'],
  installing: ['handedoff', 'current', 'failed'],
  handedoff: ['checking'],
  current: ['checking'],
  failed: ['checking', 'downloading'],
}

export function transitionUpdate(current: UpdateStatus, phase: UpdatePhase, progress = current.progress): UpdateStatus {
  if (!allowedTransitions[current.phase].includes(phase)) {
    throw new Error(`invalid update transition: ${current.phase} -> ${phase}`)
  }

  return {
    ...current,
    phase,
    progress: Math.max(0, Math.min(100, progress)),
    errorCode: phase === 'failed' ? current.errorCode : null,
  }
}
