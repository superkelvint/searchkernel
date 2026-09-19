import { describe, expect, it } from 'vitest'
import { displayFields, displayValue } from './result-display'
import type { Field } from '../api/searchkernel-client'

const field = (name: string, order: number, visible = true, format: Field['display']['format'] = 'text'): Field => ({ name, type: format, searchable: false, attribute: false, stored: true, analyzer: '—', display: { label: name.toUpperCase(), order, visible, format } })

describe('schema-driven result display', () => {
  it('orders and filters columns using display preferences', () => {
    expect(displayFields([field('hidden', 0, false), field('second', 2), field('first', 1)]).map(item => item.name)).toEqual(['first', 'second'])
  })

  it('formats values without knowing product fields', () => {
    expect(displayValue(1200, 'number')).toBe('1,200')
    expect(displayValue(true, 'boolean')).toBe('Yes')
    expect(displayValue({ state: 'ready' }, 'json')).toBe('{"state":"ready"}')
  })
})
