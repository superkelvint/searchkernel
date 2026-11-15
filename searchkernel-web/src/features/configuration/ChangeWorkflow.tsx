import { useEffect, useRef, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useNavigate, useParams, useSearchParams } from 'react-router-dom'
import type { IndexDefinitionT } from '@searchkernel/client'
import { useSearchKernelClient } from '../../app/client-context'
import { targetKey, targetName, type Target, type ConfigurationPlan, type Index } from '../../api/searchkernel-client'
import { ErrorNotice } from '../../components/Workbench'

function incarnationQueries(target: Target) {
  const name = targetName(target), generation = typeof target === 'string' ? undefined : target.generation
  return (query: { queryKey: readonly unknown[] }) => query.queryKey[1] === name && query.queryKey[2] === generation
}

export function ChangeWorkflow({ target, definition, create = false, guided = false, onApplied }: { target: Target; definition: IndexDefinitionT; create?: boolean; guided?: boolean; onApplied?: (index: Index) => void }) {
  const client = useSearchKernelClient(), queries = useQueryClient()
  const [params, setParams] = useSearchParams(), { indexId } = useParams()
  const key = targetKey(target)
  const [validated, setValidated] = useState<{ key: string; draft: IndexDefinitionT; plan: ConfigurationPlan }>()
  const [pending, setPending] = useState(false), [error, setError] = useState<unknown>()
  const [confirm, setConfirm] = useState(false), [message, setMessage] = useState('')
  const busy = useRef(false), mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  const plan = validated?.draft === definition && validated.key === key ? validated.plan : undefined
  const available = Boolean(client.planIndexChange && (create ? client.createIndex : client.applyIndexChange))
  async function validate() {
    if (busy.current) return
    busy.current = true; setPending(true); setError(undefined); setValidated(undefined); setConfirm(false); setMessage('')
    try { const plan = await client.planIndexChange!(target, definition); setValidated({ key, draft: definition, plan }) }
    catch (error) { setError(error) }
    finally { busy.current = false; setPending(false) }
  }
  async function apply() {
    if (busy.current || !plan?.supported || !confirm) return
    busy.current = true; setPending(true); setError(undefined); setValidated(undefined); setConfirm(false)
    try {
      const index = create ? await client.createIndex!(targetName(target), plan.definition) : await client.applyIndexChange!(target, plan.planId)
      queries.setQueryData(['index', index.id, index.generation], index)
      // Establish the backend-returned incarnation before invalidating the old
      // workspace; removing its active metadata query can unmount this editor.
      if (mounted.current) {
        if (indexId === index.id) { const next = new URLSearchParams(params); next.set('generation', index.generation!); setParams(next, { replace: true }) }
        onApplied?.(index)
      }
      await queries.cancelQueries({ predicate: incarnationQueries(target) })
      queries.removeQueries({ predicate: incarnationQueries(target) })
      await Promise.all([queries.invalidateQueries({ queryKey: ['indexes'] }), queries.invalidateQueries({ queryKey: ['daemon-status'] })])
      setMessage(`Applied successfully. Active generation: ${index.generation ?? 'Unavailable'}.`)
    } catch (error) { setError(error) }
    finally { busy.current = false; setPending(false) }
  }
  return <section className="surface mt-5 p-5" aria-label="Configuration change plan"><h3 className="font-semibold">{guided ? "Check fields and create" : "Validate and apply"}</h3><p className="muted mt-1">{guided ? "Search Kernel checks these fields before creating your collection. Then confirm creation to begin adding documents." : "The canonical backend validates and compiles the definition before a lifecycle change. Applying requires confirmation."}</p><button className="soft-button mt-4" disabled={pending || !available} onClick={() => void validate()}>{pending ? 'Working…' : guided ? 'Check fields' : 'Validate draft'}</button>{!available && <p className="muted mt-3">Configuration validation and apply are unavailable from this client.</p>}<ErrorNotice error={error}/>{plan && <div className="mt-4 space-y-3"><p className="text-sm font-semibold">{plan.supported ? 'Validation passed' : 'Change cannot be applied'}</p><ul className="list-disc pl-5 text-sm">{plan.reasons.map(reason => <li key={reason}>{reason}</li>)}</ul><details><summary>Technical plan details</summary><p className="muted">Validated generation: {plan.generation ?? 'New index'}. {plan.requiresReopen ? 'The active engine will close and reopen.' : 'This creates a new target.'}</p></details>{plan.supported && <><label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={confirm} onChange={event => setConfirm(event.target.checked)}/>{guided ? `Create the collection ${targetName(target)} with these checked fields.` : `I confirm applying this validated definition to ${targetName(target)}.`}</label><button className="primary-button" disabled={pending || !confirm} onClick={() => void apply()}>{create ? guided ? 'Create collection' : 'Create index' : 'Apply validated change'}</button></>}</div>}{message && <p role="status" className="mt-4 text-sm text-green-800">{message}</p>}</section>
}

export function ReopenIndex({ target }: { target: Target }) {
  const client = useSearchKernelClient(), queries = useQueryClient(), navigate = useNavigate()
  const [params] = useSearchParams(), [pending, setPending] = useState(false), [error, setError] = useState<unknown>()
  const mounted = useRef(true), busy = useRef(false)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  async function reopen() {
    if (busy.current) return
    busy.current = true; setPending(true); setError(undefined)
    try {
      const index = await client.reopenIndex!(target)
      queries.setQueryData(['index', index.id, index.generation], index)
      if (mounted.current) { const next = new URLSearchParams(params); next.set('generation', index.generation!); navigate({ search: next.toString() }, { replace: true }) }
      await queries.cancelQueries({ predicate: incarnationQueries(target) }); queries.removeQueries({ predicate: incarnationQueries(target) })
      await Promise.all([queries.invalidateQueries({ queryKey: ['indexes'] }), queries.invalidateQueries({ queryKey: ['daemon-status'] })])
    } catch (error) { setError(error) }
    finally { busy.current = false; setPending(false) }
  }
  return <div><button className="soft-button" disabled={!client.reopenIndex || pending} onClick={() => void reopen()}>{pending ? 'Reopening…' : 'Reopen index'}</button><ErrorNotice error={error}/></div>
}
