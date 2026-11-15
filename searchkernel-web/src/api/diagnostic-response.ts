import { DiagnosticExecutionKind, DiagnosticObservation, DiagnosticProvenance,
  DiagnosticSection, DiagnosticStatus, fromValue, toCanonicalJson, type QueryDiagnosticsResultT } from '@searchkernel/client'
import type { ClientDiagnostics, DiagnosticExecution, FieldValue } from './searchkernel-client'

/** Presentation conversion only; all findings and availability are backend-owned. */
export function diagnosticResponse(raw: QueryDiagnosticsResultT, display: (value: unknown) => FieldValue): ClientDiagnostics {
  const identity = (value: unknown, name: string): string => {
    if (typeof value !== 'string' || value.length === 0) throw new TypeError(`Diagnostic response is missing ${name}`)
    return value
  }
  const execution = (value: NonNullable<typeof raw.execution>): DiagnosticExecution => ({
    id: identity(value.id, 'execution ID'), kind: DiagnosticExecutionKind[value.kind], complete: value.complete,
    target: value.target?.name ? String(value.target.name) : undefined,
    generation: value.target?.generation ? String(value.target.generation) : undefined,
    requestHash: identity(value.requestHash, 'request hash'),
  })
  if (!raw.execution) throw new TypeError('Diagnostic response is missing execution identity')
  const json = (value: unknown) => JSON.stringify(toCanonicalJson(value), null, 2)
  return {
    execution: execution(raw.execution), sourceRun: raw.sourceRun ? execution(raw.sourceRun) : undefined,
    canonicalResponse: json(raw), warnings: raw.warnings,
    findings: raw.findings.map(value => ({ code: String(value.code), message: String(value.message), evidenceIds: value.evidenceIds })),
    evidence: raw.evidence.map(value => ({ id: String(value.id), section: DiagnosticSection[value.section],
      status: DiagnosticStatus[value.status], provenance: DiagnosticProvenance[value.provenance],
      executionId: String(value.executionId), message: String(value.message),
      values: Object.fromEntries(value.values.map(entry => [String(entry.key), display(fromValue(entry.value!))])),
      canonicalRequest: value.canonicalRequest ? json(value.canonicalRequest) : undefined,
      result: value.result ? json(value.result) : undefined,
    })),
    hits: raw.hits.map(value => ({ id: String(value.id), executionId: String(value.executionId),
      position: value.returnedPosition, relevance: value.relevance ? json(value.relevance) : 'Unavailable',
      signals: value.signals.map(signal => ({ name: String(signal.name), value: display(fromValue(signal.value!)),
        position: signal.position?.value, ties: signal.ties?.value, comparisonCount: signal.comparisonCount,
        comparisonSet: String(signal.comparisonSet) })),
    })),
    expectedDocument: raw.expectedDocument ? { id: String(raw.expectedDocument.id),
      stages: raw.expectedDocument.stages.map(value => ({ name: String(value.name), observation: DiagnosticObservation[value.observation], evidenceIds: value.evidenceIds })) } : undefined,
    comparison: raw.comparison ? { changed: raw.comparison.resultsChanged?.value,
      overlapCount: raw.comparison.overlapCount, comparisonCount: raw.comparison.comparisonCount,
      entering: raw.comparison.entering, leaving: raw.comparison.leaving,
      movements: raw.comparison.movements.map(value => ({ id: String(value.id), baselinePosition: value.baselinePosition?.value, rerunPosition: value.rerunPosition?.value })) } : undefined,
    rawEngine: raw.rawEngine.map(value => ({ engine: String(value.engine), version: String(value.version),
      executionId: String(value.executionId), section: DiagnosticSection[value.section], status: DiagnosticStatus[value.status],
      mediaType: String(value.mediaType), payload: String(value.payload) })),
  }
}
