import { describe, expect, it } from 'vitest'
import { translate } from './i18n'

describe('desktop translations', () => {
  it('provides the primary update and recovery actions in both languages', () => {
    for (const key of ['checkAll', 'install', 'rollbackRuntime', 'clearLogs'] as const) {
      expect(translate('zh-CN', key)).not.toBe('')
      expect(translate('en', key)).not.toBe('')
      expect(translate('zh-CN', key)).not.toBe(translate('en', key))
    }
  })
})
