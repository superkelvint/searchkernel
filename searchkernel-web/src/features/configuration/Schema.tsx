import { Heading } from '../../components/Shared'
import { useIndex } from '../../app/index-context'
import { canonicalJson } from './configuration'
import { ReopenIndex } from './ChangeWorkflow'

export function Schema() {
  const index = useIndex()
  return <div className="min-w-0">
    <Heading title="Schema" description={`Canonical index definition for ${index.id}.`}><ReopenIndex target={index.id} /></Heading>
    <section className="surface overflow-hidden"><div className="border-b border-slate-200 p-5"><h3 className="font-semibold">Fields</h3><p className="muted mt-1">Declared optional capabilities remain absent when the definition omits them.</p></div>
      <div className="overflow-auto"><table className="w-full text-left text-sm"><thead className="bg-slate-50 text-xs text-slate-500"><tr>{['Field', 'Type', 'Searchable', 'Attribute', 'Stored', 'Analyzer'].map(name => <th key={name} className="px-5 py-3">{name}</th>)}</tr></thead><tbody>{index.fields.map(field => <tr key={field.name} className="border-t border-slate-100"><td className="px-5 py-4 font-mono">{field.name}</td><td className="px-5 py-4">{field.type}</td>{[field.searchable, field.attribute, field.stored].map((value, position) => <td key={position} className="px-5 py-4">{value === undefined ? 'Not declared' : value ? 'Enabled' : 'Disabled'}</td>)}<td className="px-5 py-4">{field.analyzer}</td></tr>)}</tbody></table></div>
    </section>
    <section className="surface mt-5 p-5"><h3 className="font-semibold">Supported administration</h3><p className="muted mt-2">Create indexes with text, keyword, integer, floating point and boolean fields. Existing indexes support validated rank-profile changes with close/reopen and retained documents.</p><p className="muted mt-2">Changing existing fields, schema identity, analyzers, summaries, constants or models requires reindexing. Reindexing and live field mutation are unavailable. Rank features and advanced ranking controls are inspectable when declared, but this editor supports first-phase expressions only.</p></section>
    <section className="surface mt-5 p-5"><h3 className="font-semibold">Complete canonical definition</h3><p className="muted mt-1">Includes schema identity, inheritance, fields, fieldsets, summaries, rank profiles, constants and models. Integer values are displayed without precision loss.</p>{index.definition ? <pre className="mt-4 max-h-160 overflow-auto rounded-lg bg-slate-950 p-4 text-xs text-slate-100">{canonicalJson(index.definition)}</pre> : <p className="muted mt-4">The canonical definition is unavailable from this client. Field summaries are not a complete definition.</p>}</section>
  </div>
}
