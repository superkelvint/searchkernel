import type { Field, FieldValue, SearchHit } from '../api/searchkernel-client'

export function displayFields(fields: Field[]) { return fields.filter(field => field.display.visible).sort((left, right) => left.display.order - right.display.order) }

export function displayValue(value: FieldValue, format: Field['display']['format']): string {
  if (value === null) return '—'
  if (format === 'json' || typeof value === 'object') return JSON.stringify(value)
  if (format === 'boolean') return value ? 'Yes' : 'No'
  if (format === 'number' && typeof value === 'number') return value.toLocaleString()
  if (format === 'date' && typeof value === 'string') return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(new Date(value))
  return String(value)
}

export function displayHitValue(hit: SearchHit, field: Field) { return displayValue(hit.document.fields[field.name] ?? null, field.display.format) }
