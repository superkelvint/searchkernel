import {
  DiagnosticIntent as Intent, DiagnosticSection as Section, DiagnosticLimitsT,
  QueryDiagnosticsRequestT, toCanonicalJson,
  OptionalUInt32T, OptionalUInt64T,
  diagnosticsCurl,
} from '@searchkernel/client'
import type { DiagnosticOptions, Index, PreparedDiagnostics, PreparedSearchRequest } from './searchkernel-client'
import { SearchKernelClientError } from './searchkernel-client'
import { preparedRequest } from './search-request'

const intents = {
  'explain-hit': Intent.ExplainHit, 'zero-results': Intent.ZeroResults,
  'missing-document': Intent.MissingDocument, 'ranking-comparison': Intent.RankingComparison,
  overmatching: Intent.Overmatching, 'query-interpretation': Intent.QueryInterpretation,
  'vector-retrieval': Intent.VectorRetrieval, performance: Intent.Performance,
  'deep-engine': Intent.DeepEngine, experiment: Intent.Experiment,
}
const sections = {
  query: Section.Query, retrieval: Section.Retrieval, ranking: Section.Ranking, timing: Section.Timing,
  'matched-terms': Section.MatchedTerms, vectors: Section.Vectors, trace: Section.Trace,
  'matching-profiler': Section.MatchingProfiler, 'first-phase-profiler': Section.FirstPhaseProfiler,
  'second-phase-profiler': Section.SecondPhaseProfiler, blueprint: Section.Blueprint,
  'iterator-tree': Section.IteratorTree, 'raw-engine': Section.RawEngine, counterfactual: Section.Counterfactual,
}
const owned = new WeakMap<PreparedDiagnostics, QueryDiagnosticsRequestT>()

/** W5 never resolves a logical name after the selected incarnation is known. */
export function diagnosticTarget(index: Index): NonNullable<Index['target']> {
  if (!index.generation || !index.target) throw new SearchKernelClientError('Open the index before collecting diagnostics.', { code: 'index_not_open', origin: 'client' })
  if (String(index.target.name) !== index.id || String(index.target.generation) !== index.generation) throw new SearchKernelClientError(
    'The selected diagnostic target does not match the opened index incarnation.',
    { code: 'stale_index_handle', origin: 'client', retryable: false })
  return index.target
}

/** Maps product controls to canonical controls; no engine query manipulation. */
export function prepareDiagnostics(index: Index, search: PreparedSearchRequest, sourceRunId: string | undefined, options: DiagnosticOptions): PreparedDiagnostics {
  const target = diagnosticTarget(index)
  const limits = new DiagnosticLimitsT()
  // SDK optional scalar helpers preserve presence; validation stays in the IR.
  if (options.timeoutMs !== undefined) limits.timeoutMs = new OptionalUInt64T(BigInt(options.timeoutMs))
  if (options.traceDepth !== undefined) limits.traceDepth = new OptionalUInt32T(options.traceDepth)
  if (options.profilerDepth !== undefined) limits.profilerDepth = new OptionalUInt32T(options.profilerDepth)
  const request = new QueryDiagnosticsRequestT(preparedRequest(search), intents[options.intent],
    options.sections?.map(section => sections[section]) ?? [], sourceRunId ?? null,
    options.selectedHits ?? [], options.expectedDocumentId ?? null,
    options.experiment ? preparedRequest(options.experiment) : null, options.compareExact ?? false, limits)
  const result = Object.freeze({ target, canonical: JSON.stringify(toCanonicalJson(request), null, 2) })
  owned.set(result, request)
  return result
}
export function preparedDiagnosticRequest(value: PreparedDiagnostics): QueryDiagnosticsRequestT {
  const request = owned.get(value)
  if (!request) throw new TypeError('Diagnostics must be prepared by the Search Kernel adapter')
  return request
}

export function diagnosticCurl(value: PreparedDiagnostics, endpoint: string): string {
  return diagnosticsCurl(endpoint, String(value.target.name), String(value.target.generation), preparedDiagnosticRequest(value))
}
