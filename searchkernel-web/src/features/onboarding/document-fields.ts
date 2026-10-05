import { toCanonicalJson, value } from '@searchkernel/client'
import type { Field } from '../../api/searchkernel-client'

export const simpleField = (field: Field) => ['string', 'text', 'keyword', 'int64', 'int', 'float64', 'float', 'bool'].includes(field.type)
/** UI value entry only: SDK encoding and backend validation remain authoritative. */
export function firstDocumentFields(fields: Field[], inputs: Record<string, string>): string {
  return JSON.stringify(Object.fromEntries(fields.filter(field => inputs[field.name] !== undefined && inputs[field.name] !== '').map(field => {
    const input = inputs[field.name]
    if (!simpleField(field)) throw new TypeError(`Use the advanced editor for ${field.name}.`)
    if (['int64', 'int'].includes(field.type) && !/^[+-]?\d+$/.test(input)) throw new TypeError(`Enter a whole number for ${field.name}.`)
    if (['float64', 'float'].includes(field.type) && input.trim() === '') throw new TypeError(`Enter a finite number for ${field.name}.`)
    if (field.type === 'bool' && !['true', 'false'].includes(input)) throw new TypeError(`Choose true or false for ${field.name}.`)
    const typed = ['int64', 'int'].includes(field.type) ? BigInt(input)
      : ['float64', 'float'].includes(field.type) ? Number(input)
      : field.type === 'bool' ? input === 'true' : input
    if (typeof typed === 'number' && !Number.isFinite(typed)) throw new TypeError(`Enter a finite number for ${field.name}.`)
    return [field.name, toCanonicalJson(value(typed))]
  })), null, 2)
}
