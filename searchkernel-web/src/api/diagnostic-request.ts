import {
  DiagnosticIntent as Intent, DiagnosticSection as Section, DiagnosticLimitsT,
  QueryDiagnosticsRequestT, toCanonicalJson,
  OptionalUInt32T, OptionalUInt64T,
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

/** Maps product controls to canonical controls; no engine query manipulation. */
export function prepareDiagnostics(index: Index, search: PreparedSearchRequest, sourceRunId: string | undefined, options: DiagnosticOptions): PreparedDiagnostics {
  if (!index.generation) throw new SearchKernelClientError('Open the index before collecting diagnostics.', { code: 'index_not_open', origin: 'client' })
  const limits = new DiagnosticLimitsT()
  // SDK optional scalar helpers preserve presence; validation stays in the IR.
  if (options.timeoutMs !== undefined) limits.timeoutMs = new OptionalUInt64T(BigInt(options.timeoutMs))
  if (options.traceDepth !== undefined) limits.traceDepth = new OptionalUInt32T(options.traceDepth)
  if (options.profilerDepth !== undefined) limits.profilerDepth = new OptionalUInt32T(options.profilerDepth)
  const request = new QueryDiagnosticsRequestT(preparedRequest(search), intents[options.intent],
    options.sections?.map(section => sections[section]) ?? [], sourceRunId ?? null,
    options.selectedHits ?? [], options.expectedDocumentId ?? null,
    options.experiment ? preparedRequest(options.experiment) : null, options.compareExact ?? false, limits)
  const result = Object.freeze({ target: index.id, generation: index.generation, canonical: JSON.stringify(toCanonicalJson(request), null, 2) })
  owned.set(result, request)
  return result
}
export function preparedDiagnosticRequest(value: PreparedDiagnostics): QueryDiagnosticsRequestT {
  const request = owned.get(value)
  if (!request) throw new TypeError('Diagnostics must be prepared by the Search Kernel adapter')
  return request
}
