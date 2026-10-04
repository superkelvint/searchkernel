import { describe, it, expect } from 'vitest';
import { ByteBuffer } from 'flatbuffers';
import { Client } from '../src/client/index.js';
import { SearchRequestBuilder } from '../src/ergonomic/builders.js';
import { QueryDiagnosticsRequestT, DiagnosticIntent, DiagnosticSection, DiagnosticStatus } from '../src/semantic.js';
import { diagnosticsCurl } from '../src/client/diagnostic-export.js';
import { packObject } from '../src/_wire/v1/codec.js';
import { RequestEnvelope } from '../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/request-envelope.js';
import { ResponseEnvelopeT } from '../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/response-envelope.js';
import { ResponsePayload } from '../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/response-payload.js';
import * as Wire from '../src/_wire/v1/generated/flatbuffers/search-kernel/wire.js';

describe('public diagnostic operation', () => {
  it('round trips typed identity, partial evidence and cancellation through the bound client', async () => {
    const raw = new Wire.QueryDiagnosticsResultT();
    raw.execution = new Wire.DiagnosticExecutionT('rerun', 'hash', new Wire.IndexTargetT('catalog', 'G1'), Wire.DiagnosticExecutionKind.DiagnosticRerun, true);
    const evidence = new Wire.DiagnosticEvidenceT(); evidence.id = 'counterfactual'; evidence.section = Wire.DiagnosticSection.Counterfactual; evidence.status = Wire.DiagnosticStatus.Unsupported; evidence.executionId = 'rerun'; evidence.message = 'Unavailable in this integration';
    raw.evidence = [evidence]; raw.warnings = ['A diagnostic rerun, not the original execution'];
    const matcherPayload = JSON.stringify({ final_matcher_request: {
      format: 'searchkernel.proton-request.v1', rank_profile: 'diagnostic',
      properties: { rank: { entries: [{ name: 'private_property', value: '[redacted]' }] } },
      future_native_key: { opaque: true },
    } });
    raw.rawEngine = [new Wire.RawEngineDiagnosticsT('vespa', 'pinned-native', 'rerun', Wire.DiagnosticSection.Trace, Wire.DiagnosticStatus.Available, 'application/json', matcherPayload)];
    const bytes = packObject(new ResponseEnvelopeT(1, ResponsePayload.SearchKernel_Wire_QueryDiagnosticsResult, raw), 'SKRS');
    const controller = new AbortController();
    const client = new Client({ async execute(request, options) {
      expect(options?.signal).toBe(controller.signal);
      const envelope = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(request));
      expect(envelope.target()?.name()).toBe('catalog'); expect(envelope.target()?.generation()).toBe('G1');
      const body = envelope.payload(new Wire.QueryDiagnosticsRequest());
      expect(body?.intent()).toBe(Wire.DiagnosticIntent.ExplainHit);
      expect(body?.sourceRunId()).toBe('original');
      expect(body?.request()?.diagnosticSource()).toBe(true);
      return bytes;
    } }, 'catalog');
    client.bindGeneration('G1');
    const search = new SearchRequestBuilder('orchid').ranking('lexical').limit(10).build(); search.diagnosticSource = true;
    const request = new QueryDiagnosticsRequestT(search, DiagnosticIntent.ExplainHit, [DiagnosticSection.Counterfactual], 'original', ['a']);
    const decoded = await client.queryDiagnostics(request, { signal: controller.signal });
    expect(decoded.execution?.target?.generation).toBe('G1');
    // Preserve private native keys opaquely for Inspect and deliberate export.
    expect(decoded.rawEngine[0].payload).toBe(matcherPayload);
    expect(JSON.parse(JSON.stringify(decoded)).rawEngine[0].payload).toBe(matcherPayload);
    expect(decoded.evidence[0]?.status).toBe(DiagnosticStatus.Unsupported);
    expect(decoded.warnings).toEqual(raw.warnings);
    expect(diagnosticsCurl('http://localhost:8080', 'catalog', 'G1', request)).toContain('SearchKernel_Wire_QueryDiagnosticsRequest');
  });
});
