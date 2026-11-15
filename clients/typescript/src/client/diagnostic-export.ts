import { QueryDiagnosticsRequestT } from '../semantic.js';
import { toCanonicalJson } from './json.js';

/** Standard public HTTP/BFBS request, never a private browser diagnostic DTO. */
export function diagnosticsCurl(endpoint: string, target: string, generation: string, request: QueryDiagnosticsRequestT): string {
  const url = new URL(endpoint);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) throw new TypeError('Use a public HTTP daemon endpoint without credentials or query parameters');
  if (!generation) throw new TypeError('An opened index generation is required');
  url.pathname = `${url.pathname.replace(/\/$/, '')}/v1/execute`;
  const envelope = { protocol_version: 1, target: { name: target, generation }, payload_type: 'SearchKernel_Wire_QueryDiagnosticsRequest', payload: toCanonicalJson(request) };
  const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
  return `curl --request POST ${quote(url.toString())} --header 'Content-Type: application/json' --data-binary ${quote(JSON.stringify(envelope))}`;
}
