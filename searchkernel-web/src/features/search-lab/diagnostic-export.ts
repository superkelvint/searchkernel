import type { ClientDiagnostics, PreparedDiagnostics } from '../../api/searchkernel-client'

export function diagnosticBundle(result: ClientDiagnostics, request: PreparedDiagnostics,
  include: { query: boolean; values: boolean; raw: boolean }) {
  const canonicalRequest = include.query ? JSON.parse(request.canonical) : undefined
  if (canonicalRequest && !include.values) { delete canonicalRequest.selected_hits; delete canonicalRequest.expected_document_id }
  return JSON.stringify({ format: 'searchkernel-query-diagnostics-v1',
    searchKernelVersion: result.evidence.find(value => value.section === 'Query')?.values.searchkernel_version ?? 'Unavailable',
    execution: result.execution, sourceRun: result.sourceRun,
    engineVersions: result.rawEngine.map(value => ({ engine: value.engine, version: value.version })),
    request: canonicalRequest,
    findings: result.findings.map(value => ({ code: value.code, evidenceIds: value.evidenceIds,
      message: include.query ? value.message : undefined })),
    evidence: result.evidence.map(value => ({ id: value.id, section: value.section, status: value.status,
      provenance: value.provenance, executionId: value.executionId,
      message: include.query ? value.message : undefined,
      values: include.values && (value.section !== 'Query' || include.query) ? value.values : undefined,
      canonicalRequest: include.query && value.canonicalRequest ? JSON.parse(value.canonicalRequest) : undefined,
      result: include.values && value.result ? JSON.parse(value.result) : undefined })),
    hits: include.values ? result.hits : undefined,
    expectedDocument: include.values ? result.expectedDocument : undefined,
    comparison: include.values ? result.comparison : undefined,
    rawEngine: include.raw ? result.rawEngine : undefined,
  }, null, 2)
}

/** Numeric comparison only, within a named signal. Never cross-scale importance. */
export function signalComparison(a: unknown, b: unknown): string {
  if (typeof a === 'number' && typeof b === 'number') {
    if (a === b) return 'Tie'
    return a > b ? 'Numerically favours A' : 'Numerically favours B'
  }
  return JSON.stringify(a) === JSON.stringify(b) ? 'Equal values' : 'Different structured values; numerical ordering unavailable'
}
