import { useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import type { ClientDiagnostics, ClientSearchResponse, DiagnosticOptions, Index, PreparedDiagnostics, PreparedSearchRequest, SearchKernelClient } from '../../api/searchkernel-client'
import { normalizeClientError } from '../../api/searchkernel-client'
import { CanonicalInspector, CopyButton, ErrorNotice } from '../../components/Workbench'
import { diagnosticBundle, signalComparison } from './diagnostic-export'
import { diagnosticTarget } from '../../api/diagnostic-request'
import { ReopenIndex } from '../configuration/ChangeWorkflow'

function parseEnginePayload(payload: string): unknown {
  try { return JSON.parse(payload) } catch { return payload }
}

function EvidenceTree({ value, name = 'Root' }: { value: unknown; name?: string }) {
  const [expanded, setExpanded] = useState(false)
  if (value === null || typeof value !== 'object') return <p><strong>{name}:</strong> {JSON.stringify(value)}</p>
  const entries = Object.entries(value)
  return <details onToggle={event => setExpanded(event.currentTarget.open)}><summary>{name} · {entries.length} {Array.isArray(value) ? 'items' : 'properties'}</summary>{expanded && <div className="ml-4 border-l pl-3">{entries.map(([key, child]) => <EvidenceTree key={key} name={key} value={child}/>)}</div>}</details>
}

const problems: { label: string; intent: DiagnosticOptions['intent'] }[] = [
  { label: 'I got zero results.', intent: 'zero-results' },
  { label: 'A result I expected is missing.', intent: 'missing-document' },
  { label: 'A result is ranked too high or too low.', intent: 'ranking-comparison' },
  { label: 'Too many or irrelevant results are matching.', intent: 'overmatching' },
  { label: 'Vector/hybrid search looks wrong.', intent: 'vector-retrieval' },
  { label: 'The query is slow.', intent: 'performance' },
  { label: 'I want to understand the query.', intent: 'query-interpretation' },
]

export function QueryDiagnostics({ client, index, baseline, response, selected, initialIntent, onClose, onEvidence }:
  { client: SearchKernelClient; index: Index; baseline: PreparedSearchRequest; response: ClientSearchResponse;
    selected: string[]; initialIntent: DiagnosticOptions['intent']; onClose: () => void; onEvidence: (value: ClientDiagnostics) => void }) {
  const [tab, setTab] = useState<'Explain' | 'Investigate' | 'Inspect'>(initialIntent === 'explain-hit' || initialIntent === 'ranking-comparison' ? 'Explain' : 'Investigate')
  const [intent, setIntent] = useState(initialIntent)
  const [result, setResult] = useState<ClientDiagnostics>()
  const [prepared, setPrepared] = useState<PreparedDiagnostics>()
  const [completedRequest, setCompletedRequest] = useState<PreparedDiagnostics>()
  const [error, setError] = useState<unknown>()
  const [loading, setLoading] = useState('')
  const [lookup, setLookup] = useState('')
  const [candidates, setCandidates] = useState<ClientSearchResponse>()
  const [expected, setExpected] = useState<string>()
  const [profile, setProfile] = useState(baseline.request.ranking ?? '')
  const [inputs, setInputs] = useState(baseline.request.inputs ?? '')
  const [include, setInclude] = useState({ query: false, values: false, raw: false })
  const [httpEndpoint, setHttpEndpoint] = useState('')
  const sequence = useRef(0), abort = useRef<AbortController | null>(null)
  const scope = JSON.stringify([index.id, index.generation, response.sourceRunId, baseline.canonical])
  const currentScope = useRef(scope)
  currentScope.current = scope
  const selectedKey = JSON.stringify(selected)
  async function collect(options: DiagnosticOptions) {
    if (!client.prepareDiagnostics || !client.executeDiagnostics) return
    abort.current?.abort()
    const controller = new AbortController(), current = ++sequence.current
    abort.current = controller
    setLoading(options.intent === 'experiment' ? 'Collecting experimental rerun…' : `Collecting ${options.intent.replaceAll('-', ' ')} evidence…`)
    setError(undefined)
    try {
      const request = client.prepareDiagnostics(index, baseline, response.sourceRunId, options)
      setPrepared(request)
      const next = await client.executeDiagnostics(request, { signal: controller.signal })
      if (current !== sequence.current || scope !== currentScope.current || controller.signal.aborted) return
      if (next.execution.target !== index.id || next.execution.generation !== index.generation) throw new Error('Diagnostic evidence identity differs from the opened index')
      setResult(next); setCompletedRequest(request); onEvidence(next)
    } catch (failure) {
      if (current === sequence.current && scope === currentScope.current) setError(normalizeClientError(failure))
    } finally { if (current === sequence.current && scope === currentScope.current) setLoading('') }
  }
  useEffect(() => {
    if (initialIntent === 'ranking-comparison') setTab('Explain')
    if (initialIntent !== 'missing-document') void collect({ intent: selected.length === 2 ? 'ranking-comparison' : initialIntent, selectedHits: selected })
    return () => { ++sequence.current; abort.current?.abort() }
  }, [scope, selectedKey, initialIntent])
  async function findExpected() {
    abort.current?.abort()
    const current = ++sequence.current, controller = new AbortController()
    abort.current = controller; setLoading('Searching for the expected document…'); setError(undefined)
    try {
      const next = await client.executeSearch(diagnosticTarget(index), client.prepareSearch({ query: lookup, limit: 10 }), { signal: controller.signal })
      if (current === sequence.current && scope === currentScope.current && !controller.signal.aborted) setCandidates(next)
    } catch (failure) { if (current === sequence.current) setError(failure) }
    finally { if (current === sequence.current) setLoading('') }
  }
  const selectedHits = selected.length ? result?.hits.filter(hit => selected.includes(hit.id)) : result?.hits
  const ranking = result?.evidence.find(value => value.section === 'Ranking')
  const a = result?.hits.find(hit => hit.id === selected[0]), b = result?.hits.find(hit => hit.id === selected[1])
  const signals = new Set([...(a?.signals.map(value => value.name) ?? []), ...(b?.signals.map(value => value.name) ?? [])])
  let curl = '', curlError = ''
  if (completedRequest && httpEndpoint && client.diagnosticsCurl) {
    try { curl = client.diagnosticsCurl(completedRequest, httpEndpoint) } catch (error) { curlError = error instanceof Error ? error.message : 'Invalid HTTP endpoint' }
  }
  return <aside className="surface min-w-0 p-4" aria-label="Search evidence">
    <div className="flex items-center justify-between"><h2 className="font-semibold">Search evidence</h2><button className="soft-button" onClick={onClose}>Close evidence</button></div>
    <div className="my-3 flex gap-2" role="tablist" aria-label="Diagnostic workflow">{(['Explain', 'Investigate', 'Inspect'] as const).map((name, i, names) => <button className="soft-button" id={`diagnostic-tab-${name}`} aria-controls={`diagnostic-panel-${name}`} key={name} role="tab" tabIndex={tab === name ? 0 : -1} aria-selected={tab === name} onKeyDown={event => { if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) { event.preventDefault(); const next = event.key === 'Home' ? names[0] : event.key === 'End' ? names[2] : names[(i + (event.key === 'ArrowRight' ? 1 : 2)) % 3]; setTab(next); document.getElementById(`diagnostic-tab-${next}`)?.focus() } }} onClick={() => setTab(name)}>{name}</button>)}</div>
    {loading && <div role="status" aria-live="polite"><p>{loading} Original search results remain visible.</p><button className="soft-button" onClick={() => { ++sequence.current; abort.current?.abort(); setLoading(''); setError(new Error('Diagnostic collection cancelled. Retained evidence belongs to the preceding completed run.')) }}>Cancel diagnostics</button></div>}
    <ErrorNotice error={error}/>
    {!!error && <div className="my-3 text-sm"><p>Diagnostics remain bound to target {index.id}, generation {index.generation}. If this incarnation changed, reopen explicitly; operations are never silently retargeted.</p><ReopenIndex target={index.id}/></div>}

    {result && <div className="my-3 rounded border p-3 text-sm"><strong>{result.execution.kind === 'Experiment' ? 'Experimental rerun' : result.execution.kind === 'Counterfactual' ? 'Counterfactual diagnostic rerun — not the original execution' : 'Diagnostic rerun — not the original execution'}</strong><p>Run {result.execution.id} · {result.execution.complete ? 'Complete' : 'Incomplete; do not treat this as a complete explanation.'}</p>{result.sourceRun && <p>Original run: {result.sourceRun.id}</p>}{result.comparison?.changed && <p role="status">Results changed from the original search. Overlap: {result.comparison.overlapCount}/{result.comparison.comparisonCount} returned documents.</p>}{result.warnings.map((warning, i) => <p key={i}>{warning}</p>)}</div>}
    {tab === 'Explain' && <div role="tabpanel" id="diagnostic-panel-Explain" aria-labelledby="diagnostic-tab-Explain">
      {!selected.length && <p>Select a result to ask “Why this result?”, or enable the Explain lens to scan named signals.</p>}
      {ranking && <p className="my-3">{ranking.message}</p>}
      {ranking && <section aria-label="Configured ranking pipeline"><h3 className="font-semibold">Configured ranking phases</h3><ul>{[['First phase', 'first_phase_configured'], ['Second phase', 'second_phase_configured'], ['Global phase', 'global_phase_configured']].map(([name, key]) => <li key={key}>{name}: {ranking.values[key] === true ? 'Configured' : ranking.values[key] === false ? 'Not configured' : 'Unavailable'}</li>)}</ul><p className="text-xs">First phase scores matching candidates; second phase rescores a selected subset; global phase rescores merged results when supported. Configuration is a fact, not a problem or proof of per-hit participation.</p></section>}
      {selectedHits?.map(hit => <section key={hit.id} className="my-3 border-t pt-3"><h3 className="font-semibold">Why {hit.id}?</h3><p>Returned position #{hit.position} · final relevance: {hit.relevance}</p><p>Active profile: {String(ranking?.values.profile ?? 'Unavailable')}</p><p>Phase participation and phase-specific scores: {ranking?.values.per_phase_participation == null ? 'Unavailable' : JSON.stringify(ranking.values.per_phase_participation)}</p>{hit.signals.length > 0 ? <table className="mt-3 w-full text-left text-xs"><caption>Named signals in the diagnostic returned set</caption><thead><tr><th>Signal</th><th>Value</th><th>Context</th></tr></thead><tbody>{hit.signals.map(signal => <tr key={signal.name}><th>{signal.name}</th><td><pre className="max-w-64 overflow-auto">{JSON.stringify(signal.value, null, 2)}</pre></td><td>{signal.position === undefined ? 'Ordinal unavailable' : `#${signal.position}`}{signal.ties !== undefined && ` · ${signal.ties} tied`} among {signal.comparisonCount} returned values<br/><small>{signal.comparisonSet}</small></td></tr>)}</tbody></table> : <p>Named semantic explanation values are unavailable for this hit. This is not an error.</p>}</section>)}
      {selected.some(id => result && !result.hits.some(hit => hit.id === id)) && <p>The selected original hit is absent from the bounded diagnostic returned set. Evidence for it is unavailable in this rerun.</p>}
      <Link className="text-violet-700 underline" to={`/indexes/${encodeURIComponent(index.id)}/configuration/ranking/${encodeURIComponent(String(ranking?.values.profile ?? baseline.request.ranking ?? ''))}`}>Make this profile easier to explain</Link><p className="mt-2 text-xs">Intentionally expose named functions using match/summary features in the ranking profile. Names and observed values help comparison; they do not establish causal importance.</p>
      {selected.length === 2 && <section className="mt-4"><h3 className="font-semibold">Compare A: {selected[0]} with B: {selected[1]}</h3><p>Both original hits remain in the result list. Values below belong to the diagnostic rerun.</p><table className="w-full text-left text-xs"><thead><tr><th>Signal</th><th>A</th><th>B</th><th>Comparison</th></tr></thead><tbody>{[...signals].map(name => { const left = a?.signals.find(value => value.name === name), right = b?.signals.find(value => value.name === name); return <tr key={name}><th>{name}</th><td>{left ? JSON.stringify(left.value) : 'Unavailable'}<p>#{left?.position ?? 'unavailable'} among {left?.comparisonCount ?? 0}; ties {left?.ties ?? 'unavailable'}</p></td><td>{right ? JSON.stringify(right.value) : 'Unavailable'}<p>#{right?.position ?? 'unavailable'} among {right?.comparisonCount ?? 0}; ties {right?.ties ?? 'unavailable'}</p></td><td>{left && right ? signalComparison(left.value, right.value) : 'Comparison unavailable'}</td></tr> })}</tbody></table><p className="text-xs">Numerical favour does not explain why A ranked above B. Phase movement is unavailable unless explicitly observed.</p></section>}
    </div>}
    {tab === 'Investigate' && <div role="tabpanel" id="diagnostic-panel-Investigate" aria-labelledby="diagnostic-tab-Investigate"><h3 className="font-semibold">What’s wrong with this search?</h3><div className="my-3 grid gap-2">{problems.map(problem => <button className="soft-button text-left" key={problem.intent} disabled={!!loading} onClick={() => { setIntent(problem.intent); if (problem.intent !== 'missing-document') void collect({ intent: problem.intent, selectedHits: selected }) }}>{problem.label}</button>)}</div>
      {intent === 'missing-document' && <section className="my-3"><h3>Find the expected document</h3><label>Search/browse this index<input aria-label="Find expected document" className="input" value={lookup} onChange={e => setLookup(e.target.value)}/></label><button className="soft-button mt-2" disabled={!!loading} onClick={() => void findExpected()}>Find documents</button>{candidates?.hits.map(hit => <button key={hit.document.id} className="soft-button m-1" onClick={() => setExpected(hit.document.id)}>{hit.document.id} · {JSON.stringify(hit.document.fields)}</button>)}<details><summary>Use a known canonical document ID</summary><input className="input" aria-label="Expected document ID" value={expected ?? ''} onChange={e => setExpected(e.target.value || undefined)}/></details>{expected && <p>Expected document: {expected}</p>}<button className="soft-button mt-2" disabled={!expected || !!loading} onClick={() => void collect({ intent: 'missing-document', expectedDocumentId: expected })}>Investigate expected document</button></section>}
      {result?.findings.length ? <section aria-label="Diagnostic findings"><h3 className="font-semibold">Findings</h3>{result.findings.map((finding, i) => <div key={i} className="my-2 border p-3"><p>{finding.message}</p><p className="text-xs">Evidence: {finding.evidenceIds.join(', ')}</p>{finding.evidenceIds.map(id => { const item = result.evidence.find(item => item.id === id); return item ? <details key={id}><summary>{item.id} · {item.status}</summary><p>{item.message}</p><pre className="overflow-auto text-xs">{JSON.stringify(item.values, null, 2)}</pre>{item.canonicalRequest && <CanonicalInspector label={`${id} canonical request`} text={item.canonicalRequest}/ >}{item.result && <CanonicalInspector label={`${id} result sample`} text={item.result}/>}</details> : null })}</div>)}</section> : result && <p>No evidence-backed finding was established. Inspect the observed evidence and unavailable sections.</p>}
      {result?.expectedDocument && <section><h3 className="font-semibold">Expected document stage evidence</h3>{Boolean(result.evidence.find(item => item.id === 'expected-document')?.values.first_observed_negative_stage) && <p><strong>First observed negative stage: {String(result.evidence.find(item => item.id === 'expected-document')?.values.first_observed_negative_stage)}</strong></p>}<ol>{result.expectedDocument.stages.map(stage => <li key={stage.name}>{stage.name}: {stage.observation} {stage.evidenceIds.length > 0 && `(${stage.evidenceIds.join(', ')})`}</li>)}</ol><p className="text-xs">Unavailable intermediate stages are not inferred from “not returned”. Existence and returned status can be observed at different instants.</p></section>}
      {intent === 'vector-retrieval' && <div className="my-3"><p>Exact comparison may be expensive. The backend bounds work and constructs the exact variant.</p><button className="soft-button" disabled={!!loading} onClick={() => void collect({ intent: 'vector-retrieval', sections: ['vectors'], compareExact: true })}>Compare with exact retrieval</button></div>}
      {intent === 'performance' && <div className="my-3"><p>Profilers measure a diagnostic rerun. Matching searches for candidates; first phase scores them; second phase rescores selected candidates.</p>{(['matching-profiler', 'first-phase-profiler', 'second-phase-profiler', 'trace'] as const).map(section => <button key={section} className="soft-button m-1" disabled={!!loading} onClick={() => void collect({ intent: 'performance', sections: [section] })}>Collect {section.replaceAll('-', ' ')}</button>)}</div>}
      <details className="mt-4"><summary>Query-local experiment</summary><p className="text-sm">Changes apply only to an experimental query. Persisted profiles and documents are unchanged.</p><label>Experimental ranking profile<input aria-label="Experimental ranking profile" className="input" value={profile} onChange={e => setProfile(e.target.value)} list="rank-profiles"/></label><label>Experimental query inputs<textarea aria-label="Experimental query inputs" className="input font-mono text-xs" value={inputs} onChange={e => setInputs(e.target.value)}/></label><button className="soft-button mt-2" disabled={!!loading} onClick={() => { try { const experiment = client.prepareSearch({ ...baseline.request, ranking: profile || undefined, inputs: inputs || undefined }); void collect({ intent: 'experiment', experiment }) } catch (error) { setError(error) } }}>Run experiment</button><button className="soft-button m-2" onClick={onClose}>Return to original run</button></details>
    </div>}
    {tab === 'Inspect' && <div role="tabpanel" id="diagnostic-panel-Inspect" aria-labelledby="diagnostic-tab-Inspect"><p className="text-xs">Query processing → Retrieval/matching → Ranking → Fill/summary. Only explicitly configured/observed evidence is shown.</p>{result?.evidence.map(item => <details key={item.id} className="my-3 rounded border p-3"><summary>{item.section} · {item.status} · {item.provenance}</summary><p>{item.message}</p><p className="text-xs">Evidence {item.id} · execution {item.executionId}</p><EvidenceTree value={item.values}/>{item.canonicalRequest && <CanonicalInspector label={`${item.id} normalized request`} text={item.canonicalRequest}/ >}</details>)}<button className="soft-button mt-2" disabled={!!loading} onClick={() => void collect({ intent: 'deep-engine', sections: ['trace', 'blueprint', 'iterator-tree', 'raw-engine'] })}>Inspect Vespa internals</button><p className="mt-2 text-xs">Blueprint: an engine execution plan. Iterator tree: the matching cursors used to traverse candidates. Profiler: measured execution work, collected only on request.</p>{result?.rawEngine.map((item, i) => <details key={i} className="mt-3 border p-3"><summary>{item.engine} internals · {item.section} · {item.status}</summary><p className="text-xs">Engine-specific unstable evidence · version {item.version} · execution {item.executionId}</p>{item.mediaType === 'application/json' ? <EvidenceTree value={parseEnginePayload(item.payload)}/> : <pre className="overflow-auto text-xs">{item.payload}</pre>}<CanonicalInspector label="Raw engine payload" text={item.payload}/></details>)}</div>}
    {prepared && <CanonicalInspector label="Diagnostic request" text={prepared.canonical}/ >}
    {tab === 'Investigate' && intent === 'performance' && <section aria-label="Measured diagnostic timing"><h3 className="font-semibold">Measured rerun timing</h3><p>Matching elapsed time includes ranking. Profiler totals sum engine-thread measurements and are distinct from query wall-clock latency.</p><pre className="overflow-auto text-xs">{JSON.stringify(result?.evidence.find(item => item.section === 'Timing')?.values ?? {}, null, 2)}</pre><button className="soft-button" disabled={!!loading} onClick={() => void collect({ intent: 'performance', sections: ['matching-profiler', 'first-phase-profiler', 'second-phase-profiler'] })}>Compare measured ranking phases</button></section>}
    {result && <CanonicalInspector label="Diagnostic response" text={result.canonicalResponse}/ >}
    {completedRequest && client.diagnosticsCurl && <details className="mt-4"><summary>Copy as curl</summary><p>This explicitly includes query/input values. It uses the public HTTP daemon listener, which may differ from the browser listener. Credentials and auth headers are excluded.</p><label>Public HTTP daemon endpoint<input className="input" aria-label="Public HTTP daemon endpoint" value={httpEndpoint} placeholder="http://localhost:8080" onChange={e => setHttpEndpoint(e.target.value)}/></label>{curlError && <p role="alert">{curlError}</p>}{curl && <CopyButton label="Copy as curl" text={curl}/>}</details>}
    {completedRequest && completedRequest !== prepared && <CanonicalInspector label="Completed diagnostic request" text={completedRequest.canonical}/ >}
    {result && completedRequest && <details className="mt-4"><summary>Share / export diagnostic bundle</summary><p>Defaults exclude query/input values, document/feature values and raw engine payloads. Including these may disclose sensitive information.</p>{(['query', 'values', 'raw'] as const).map(key => <label className="block" key={key}><input type="checkbox" checked={include[key]} onChange={e => setInclude({ ...include, [key]: e.target.checked })}/> Include {key === 'query' ? 'query and query-input values' : key === 'values' ? 'document IDs and feature/document values' : 'raw engine evidence (may include query or document values)'}</label>)}<CopyButton label="Copy diagnostic bundle" text={diagnosticBundle(result, completedRequest, include)}/></details>}
  </aside>
}
