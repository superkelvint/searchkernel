import type { Target } from '../../api/searchkernel-client'
import { useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import type { IndexDefinitionT } from '@searchkernel/client'
import { Heading } from '../../components/Shared'
import { useIndex } from '../../app/index-context'
import { useCapabilities } from '../../app/client-context'
import { canonicalJson, withFirstPhase } from '../configuration/configuration'
import { ChangeWorkflow, ReopenIndex } from '../configuration/ChangeWorkflow'

export function Ranking() {
  const index = useIndex(); const { capabilities } = useCapabilities()
  const { profile: routeProfile } = useParams(); const navigate = useNavigate()
  const [creating, setCreating] = useState(false)
  const name = routeProfile ?? index.profiles[0]
  const profile = index.definition?.rankProfiles.find(profile => profile.name === name)
  const writable = capabilities?.ranking.write && capabilities.schema.changePlan && Boolean(index.definition)
  return <div className="min-w-0">
    <Heading title="Ranking" description="Inspect canonical profiles and edit supported first-phase expressions."><ReopenIndex target={index.target ?? index.id} /><button className="primary-button" disabled={!writable} onClick={() => setCreating(true)}>Create profile</button></Heading>
    <div className="grid gap-5 xl:grid-cols-[240px_1fr]"><section className="surface p-3"><p className="kicker px-2 py-1">Rank profiles</p>{index.profiles.map(profile => <button key={profile} className={`section-link font-mono ${name === profile && !creating ? 'section-link-active' : ''}`} onClick={() => { setCreating(false); navigate(`/indexes/${encodeURIComponent(index.id)}/configuration/ranking/${encodeURIComponent(profile)}?generation=${encodeURIComponent(index.generation ?? '')}`) }}>{profile}</button>)}{index.profiles.length === 0 && <p className="muted p-2">No profiles declared.</p>}</section>
      <section className="surface p-5"><h3 className="font-mono font-semibold">{creating ? 'New rank profile' : name ?? 'No selected profile'}</h3>{!creating && (profile ? <pre className="mt-4 max-h-110 overflow-auto rounded-lg bg-slate-950 p-4 text-xs text-slate-100">{canonicalJson(profile)}</pre> : <p className="muted mt-3">The selected canonical profile definition is unavailable.</p>)}
        {writable && index.definition && (creating || profile) && <RankEditor key={`${index.id}:${index.generation}:${creating ? 'new' : name}`} target={index.target ?? index.id} definition={index.definition} name={creating ? '' : String(profile!.name)} expression={creating ? '' : typeof profile!.firstPhase?.expression === 'string' ? profile!.firstPhase.expression : ''} creating={creating} onApplied={() => setCreating(false)} />}
      </section>
    </div>
    <p className="muted mt-5">Supported editor: create profiles and edit first-phase expressions. Other declared profile settings are preserved. Inheritance, inputs, functions, later phases, feature lists, ONNX models and rank properties have no editor controls in this slice. Backend validation reports unsupported definitions explicitly.</p>
  </div>
}

function RankEditor({ target, definition, name: initialName, expression: initialExpression, creating, onApplied }: { target: Target; definition: IndexDefinitionT; name: string; expression: string; creating: boolean; onApplied: () => void }) {
  const [name, setName] = useState(initialName); const [expression, setExpression] = useState(initialExpression)
  const draft = useMemo(() => withFirstPhase(definition, name, expression, creating), [definition, name, expression, creating])
  return <div className="mt-5"><label className="block text-sm font-medium">Profile name<input className="input mt-2" readOnly={!creating} value={name} onChange={event => setName(event.target.value)} /></label><label className="mt-4 block text-sm font-medium">First-phase expression<textarea className="input mt-2 min-h-24 font-mono" value={expression} onChange={event => setExpression(event.target.value)} /></label><ChangeWorkflow target={target} definition={draft} onApplied={onApplied} /></div>
}
