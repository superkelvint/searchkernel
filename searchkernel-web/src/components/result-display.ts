import type { Field, FieldValue, SearchHit } from '../api/searchkernel-client'
import { displayPreferencesFor, type DashboardDisplayPreferences, type DashboardFieldDisplay } from '../app/display-preferences'

export type DisplayField = Field & { display: DashboardFieldDisplay }

function defaultFormat(field: Field): DashboardFieldDisplay['format'] {
  if (field.type === 'boolean') return 'boolean'
  if (field.type === 'date') return 'date'
  if (field.type === 'integer' || field.type === 'float' || field.type === 'double') return 'number'
  return 'text'
}

export function displayFields(indexId: string, fields: Field[], preferences?: DashboardDisplayPreferences): DisplayField[] {
  const overrides = displayPreferencesFor(indexId, preferences)
  return fields.map((field, position) => ({
    ...field,
    display: {
      label: field.name,
      visible: true,
      order: position,
      format: defaultFormat(field),
      ...overrides[field.name],
    },
  })).filter(field => field.display.visible).sort((left, right) => left.display.order - right.display.order)
}

export function displayValue(value: FieldValue, format: DashboardFieldDisplay['format']): string {
  if (value === null) return '—'
  if (format === 'json' || typeof value === 'object') return JSON.stringify(value)
  if (format === 'boolean') return value ? 'Yes' : 'No'
  if (format === 'number' && typeof value === 'number') return value.toLocaleString()
  if (format === 'date' && typeof value === 'string') return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(new Date(value))
  return String(value)
}

export function displayHitValue(hit: SearchHit, field: DisplayField) { return displayValue(hit.document.fields[field.name] ?? null, field.display.format) }
