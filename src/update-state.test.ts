import { describe, expect, it } from 'vitest'
import { transitionUpdate } from './update-state'
import type { UpdateStatus } from './types'

const idle: UpdateStatus = {
  component: 'runtime',
  currentVersion: '0.1.0',
  availableVersion: null,
  channel: 'stable',
  phase: 'idle',
  progress: 0,
  requiresRestart: false,
  errorCode: null,
  rollbackAvailable: false,
  releaseNotes: null,
}

describe('transitionUpdate', () => {
  it('accepts a check and clamps progress', () => {
    expect(transitionUpdate(idle, 'checking', 120)).toMatchObject({ phase: 'checking', progress: 100 })
  })

  it('rejects an install without a download', () => {
    expect(() => transitionUpdate(idle, 'installing')).toThrow('idle -> installing')
  })
})
