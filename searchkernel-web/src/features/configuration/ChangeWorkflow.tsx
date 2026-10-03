import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type { IndexDefinitionT } from '@searchkernel/client'
import { useSearchKernelClient } from '../../app/client-context'
import { normalizeClientError, type ConfigurationPlan, type Index } from '../../api/searchkernel-client'
import { canonicalJson } from './configuration'

export function ChangeWorkflow({ target, definition, onApplied }: { target: string; definition: IndexDefinitionT; onApplied?: (index: Index) => void }) {
  const client = useSearchKernelClient()
  const queries = useQueryClient()
  const [validated, setValidated] = useState<{ target: string; draft: IndexDefinitionT; plan: ConfigurationPlan }>()
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<ReturnType<typeof normalizeClientError>>()
  const [confirm, setConfirm] = useState(false)
  const [message, setMessage] = useState('')
  // Any draft/target change invalidates the previous validation immediately.
  const plan = validated?.draft === definition && validated.target === target ? validated.plan : undefined
  const available = Boolean(client.planIndexChange && client.applyIndexChange)
  async function validate() {
    setPending(true); setError(undefined); setValidated(undefined); setConfirm(false); setMessage('')
    try { const plan = await client.planIndexChange!(target, definition); setValidated({ target, draft: definition, plan }) }
    catch (error) { setError(normalizeClientError(error)) }
    finally { setPending(false) }
  }
  async function apply() {
    if (!plan?.supported) return
    setPending(true); setError(undefined); setValidated(undefined); setConfirm(false)
    try {
      const index = await client.applyIndexChange!(target, plan.planId)
      // Clear results produced under the old incarnation before hydrating views.
      queries.removeQueries({ predicate: query => query.queryKey.includes(target) && query.queryKey[0] !== 'index' })
      queries.setQueryData(['index', target], index)
      await queries.invalidateQueries({ queryKey: ['indexes'] })
      setMessage(`Applied successfully. Active generation: ${index.generation ?? 'Unavailable'}.`)
      onApplied?.(index)
    } catch (error) { setError(normalizeClientError(error)) }
    finally { setPending(false) }
  }
  return <section className="surface mt-5 p-5" aria-label="Configuration change plan">
    <h3 className="font-semibold">Validate and apply</h3>
    <p className="muted mt-1">Validation checks the canonical definition and compiles it with the backend before any change to this index. Applying a plan requires confirmation.</p>
    <button className="soft-button mt-4" disabled={pending || !available} onClick={() => void validate()}>{pending ? 'Working…' : 'Validate draft'}</button>
    {!available && <p className="muted mt-3">Configuration validation and apply are unavailable from this client.</p>}
    {error && <div role="alert" className="mt-4 text-sm text-red-700"><p>{error.code}: {error.message}</p>{error.details && <pre className="mt-2 overflow-auto">{canonicalJson(error.details)}</pre>}<p>Apply is disabled. Validate again; reopen the index if its generation has changed.</p></div>}
    {plan && <div className="mt-4 space-y-3"><p className="text-sm font-semibold">{plan.supported ? 'Validation passed' : 'Change cannot be applied'}</p><ul className="list-disc pl-5 text-sm">{plan.reasons.map(reason => <li key={reason}>{reason}</li>)}</ul><p className="muted">Validated generation: {plan.generation ?? 'New index'}. {plan.requiresReopen ? 'The active engine will close and reopen.' : 'This creates a new target.'}</p>{plan.supported && <><label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={confirm} onChange={event => setConfirm(event.target.checked)} />I confirm applying this validated definition to {target}.</label><button className="primary-button" disabled={pending || !confirm} onClick={() => void apply()}>Apply validated change</button></>}</div>}
    {message && <p role="status" className="mt-4 text-sm text-green-800">{message}</p>}
  </section>
}

export function ReopenIndex({ target }: { target: string }) {
  const client = useSearchKernelClient(); const queries = useQueryClient()
  const [pending, setPending] = useState(false); const [error, setError] = useState('')
  async function reopen() {
    setPending(true); setError('')
    try {
      const index = await client.reopenIndex!(target)
      queries.removeQueries({ predicate: query => query.queryKey.includes(target) && query.queryKey[0] !== 'index' })
      queries.setQueryData(['index', target], index)
    } catch (error) { setError(normalizeClientError(error).message) }
    finally { setPending(false) }
  }
  return <div><button className="soft-button" disabled={!client.reopenIndex || pending} onClick={() => void reopen()}>{pending ? 'Reopening…' : 'Reopen index'}</button>{error && <p role="alert" className="mt-2 text-sm text-red-700">{error}</p>}</div>
}
