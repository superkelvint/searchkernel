import { describe, expect, it } from 'vitest'
import { displayFields, displayValue } from './result-display'
import type { Field } from '../api/searchkernel-client'

const field = (name: string, type = 'string'): Field => ({ name, type, searchable: false, attribute: false, stored: true, analyzer: '—' })

describe('schema-driven result display', () => {
  it('orders and filters columns using display preferences', () => {
    expect(displayFields('test', [field('hidden'), field('second'), field('first')], { connectionId: 'test', indexes: { test: { hidden: { visible: false }, second: { order: 2 }, first: { order: 1 } } } }).map(item => item.name)).toEqual(['first', 'second'])
  })

  it('formats values without knowing product fields', () => {
    expect(displayValue(1200, 'number')).toBe('1,200')
    expect(displayValue(true, 'boolean')).toBe('Yes')
    expect(displayValue({ state: 'ready' }, 'json')).toBe('{"state":"ready"}')
  })
})
