import { useMemo, useState } from 'react'
import { fromCanonicalJson, toCanonicalJson, type IndexDefinitionT } from '@searchkernel/client'
import { DefinitionEditor } from './DefinitionEditor'
import { ErrorNotice } from '../../components/Workbench'
import { Link, useNavigate } from 'react-router-dom'
import { Heading } from '../../components/Shared'
import { useCapabilities } from '../../app/client-context'
import { ChangeWorkflow } from './ChangeWorkflow'
import { canonicalJson, newDefinition, type NewField } from './configuration'

const initialField: NewField = { name: 'title', type: 'text', stored: true, searchable: true, attribute: false }
export function CreateIndex() {
  const { capabilities } = useCapabilities(); const navigate = useNavigate()
  const [target, setTarget] = useState(''); const [name, setName] = useState('')
  const [fields, setFields] = useState<NewField[]>([initialField])
  const structured = useMemo(() => newDefinition(name, fields), [name, fields])
  const [raw, setRaw] = useState<string>()
  const decoded = useMemo(() => { try { return { definition: raw === undefined ? structured : fromCanonicalJson('IndexDefinition', JSON.parse(raw)) as IndexDefinitionT } } catch (error) { return { error } } }, [raw, structured])
  const draft = decoded.definition
  function change(position: number, patch: Partial<NewField>) { setFields(fields => fields.map((field, index) => index === position ? { ...field, ...patch } : field)) }
  return <main className="mx-auto max-w-5xl p-8"><Heading title="Create index" description="Build a canonical definition, validate it with the backend, and apply the plan." /><Link className="text-sm text-indigo-700" to="/indexes">Back to indexes</Link>{capabilities?.indexes.create && capabilities.schema.changePlan ? <>
    <section className="surface mt-5 space-y-4 p-5"><label className="block text-sm font-medium">Index target<input className="input mt-2" value={target} onChange={event => setTarget(event.target.value)} placeholder="catalog" /></label><p className="muted">A logical index name; the backend validates it. Existing targets are never overwritten.</p><label className="block text-sm font-medium">Schema name<input className="input mt-2" value={name} onChange={event => setName(event.target.value)} placeholder="catalog_schema" /></label><h3 className="font-semibold">Fields</h3>{fields.map((field, position) => <div key={position} className="flex flex-wrap items-end gap-3 rounded-lg border border-slate-200 p-3"><label className="text-sm">Name<input aria-label={`Field ${position + 1} name`} className="input mt-1" value={field.name} onChange={event => change(position, { name: event.target.value })} /></label><label className="text-sm">Type<select className="input mt-1" value={field.type} onChange={event => change(position, { type: event.target.value as NewField['type'], searchable: event.target.value === 'text' && field.searchable, attribute: event.target.value !== 'text' })}>{['text', 'keyword', 'int', 'float', 'bool'].map(type => <option key={type}>{type}</option>)}</select></label>{(['stored', 'searchable', 'attribute'] as const).map(flag => <label key={flag} className="flex items-center gap-2 text-sm"><input type="checkbox" checked={field[flag]} disabled={flag === 'searchable' && field.type !== 'text'} onChange={event => change(position, { [flag]: event.target.checked })} />{flag}</label>)}<button className="soft-button" onClick={() => setFields(fields => fields.filter((_, index) => index !== position))}>Remove</button></div>)}<button className="soft-button" onClick={() => setFields(fields => [...fields, { ...initialField, name: '' }])}>Add field</button><p className="muted">This editor supports primitive fields and storage, text search and attribute flags. Complex types, analyzer customization, indexing scripts and schema inheritance have no creation controls in this slice.</p></section>
    <section className="surface mt-5 p-5"><button className="soft-button" onClick={() => setRaw(raw === undefined ? JSON.stringify(toCanonicalJson(structured), null, 2) : undefined)}>{raw === undefined ? 'Edit canonical definition' : 'Use field editor'}</button>{raw !== undefined && <DefinitionEditor value={raw} onChange={setRaw}/>}<ErrorNotice error={decoded.error}/></section>
    <details className="surface mt-5 p-5"><summary>Canonical draft</summary><pre className="mt-4 overflow-auto text-xs">{canonicalJson(draft)}</pre></details>{draft && <ChangeWorkflow create target={target} definition={draft} onApplied={index => navigate(`/indexes/${encodeURIComponent(index.id)}/configuration/schema/fields?generation=${encodeURIComponent(index.generation!)}`)} />}
  </> : <p className="surface mt-5 p-5">Index creation and configuration planning are unavailable from this client.</p>}</main>
}
