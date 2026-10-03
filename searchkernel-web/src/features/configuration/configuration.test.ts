import { describe, expect, it } from 'vitest'
import { IndexDefinitionT, RankPhaseDefinitionT, RankProfileDefinitionT, Text } from '@searchkernel/client'
import { canonicalJson, newDefinition, withFirstPhase } from './configuration'

describe('canonical configuration drafts', () => {
  it('retains incomplete field names for backend validation while editing', () => {
    const definition = newDefinition('schema', [{ name: '', type: 'text', stored: true, searchable: true, attribute: false }])
    expect(definition.fields[0]?.name).toBe('')
  })
  it('preserves all untouched canonical properties and optional phase settings', () => {
    const definition = new IndexDefinitionT('schema', [new Text().definition('title')])
    const profile = new RankProfileDefinitionT('custom', ['parent'])
    profile.firstPhase = new RankPhaseDefinitionT('1')
    profile.matchFeatures = ['bm25(title)']
    definition.rankProfiles = [profile]
    const changed = withFirstPhase(definition, 'custom', '2')
    expect(changed).toBeInstanceOf(IndexDefinitionT)
    expect(changed.fields).toBe(definition.fields)
    expect(changed.rankProfiles[0]).toBeInstanceOf(RankProfileDefinitionT)
    expect(changed.rankProfiles[0]?.inherits).toBe(profile.inherits)
    expect(changed.rankProfiles[0]?.matchFeatures).toBe(profile.matchFeatures)
    expect(changed.rankProfiles[0]?.firstPhase?.expression).toBe('2')
    expect(changed.rankProfiles[0]?.firstPhase?.rerankCount).toBeNull()
    expect(profile.firstPhase.expression).toBe('1')
  })
  it('leaves duplicate creation for canonical validation and never turns it into an edit', () => {
    const definition = new IndexDefinitionT('schema')
    definition.rankProfiles = [new RankProfileDefinitionT('custom')]
    const draft = withFirstPhase(definition, 'custom', '2', true)
    expect(draft.rankProfiles).toHaveLength(2)
    expect(definition.rankProfiles).toHaveLength(1)
  })
  it('uses canonical builders and preserves explicit false attributes', () => {
    const definition = newDefinition('schema', [{ name: 'title', type: 'text', stored: true, searchable: true, attribute: false }])
    expect(definition.fields[0]?.attribute?.enabled?.value).toBe(false)
    expect(definition.fields[0]?.searchable?.value).toBe(true)
  })
  it('inspects canonical data without codec bookkeeping or integer rounding', () => {
    const definition = new IndexDefinitionT('schema')
    const display = canonicalJson({ definition, integer: 9007199254740993n })
    expect(display).not.toContain('_semanticFields')
    expect(display).not.toContain('_schemaName')
    expect(display).toContain('9007199254740993')
    expect(JSON.parse(display).definition).toHaveProperty('rankProfiles', [])
  })
})
