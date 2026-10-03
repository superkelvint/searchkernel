import { AttributeConfigT, OptionalBoolT, IndexDefinitionT, RankPhaseDefinitionT, RankProfileDefinitionT, Text, Keyword, Integer, Float, Boolean } from '@searchkernel/client'

/** Display canonical SDK values without the SDK's codec bookkeeping. */
export function canonicalJson(value: unknown): string {
  return JSON.stringify(value, (key, item: unknown) => {
    if (key === '_schemaName' || key === '_semanticFields') return undefined
    if (typeof item === 'bigint') return { $type: 'integer', decimal: item.toString() }
    if (item instanceof Uint8Array) return { $type: 'bytes', values: Array.from(item) }
    return item
  }, 2)
}

/** Preserve every untouched canonical property, including optional presence. */
export function withFirstPhase(definition: IndexDefinitionT, name: string, expression: string, creating = false): IndexDefinitionT {
  const draft = Object.assign(new IndexDefinitionT(), definition)
  const existing = creating ? undefined : definition.rankProfiles.find(profile => profile.name === name)
  const profile = Object.assign(new RankProfileDefinitionT(), existing, { name })
  profile.firstPhase = Object.assign(new RankPhaseDefinitionT(), existing?.firstPhase, { expression })
  draft.rankProfiles = existing
    ? definition.rankProfiles.map(value => value === existing ? profile : value)
    : [...definition.rankProfiles, profile]
  return draft
}

export type NewField = { name: string; type: 'text' | 'keyword' | 'int' | 'float' | 'bool'; stored: boolean; searchable: boolean; attribute: boolean }
export function newDefinition(name: string, fields: NewField[]): IndexDefinitionT {
  const builders = { text: Text, keyword: Keyword, int: Integer, float: Float, bool: Boolean }
  return new IndexDefinitionT(name, fields.map(field => {
    const Builder = builders[field.type]
    // Builders require a name, but an administration draft can be incomplete.
    // Preserve that draft in the canonical table for backend validation.
    const definition = new Builder({ stored: field.stored, searchable: field.searchable }).definition(field.name || 'draft')
    definition.name = field.name
    definition.attribute = new AttributeConfigT(new OptionalBoolT(field.attribute))
    return definition
  }))
}
