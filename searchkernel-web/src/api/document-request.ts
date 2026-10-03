import { fromCanonicalJson, jsonInput, toCanonicalJson, value, type OperationRequestMap } from '@searchkernel/client'
import { SearchKernelClientError, type DocumentEditorState, type DocumentMutationOperation, type PreparedDocumentMutation } from './searchkernel-client'

const tables = { insert: 'InsertRequest', upsert: 'UpsertRequest', patch: 'PatchRequest', update: 'UpdateRequest', delete: 'DeleteRequest' } as const
const requests = new WeakMap<PreparedDocumentMutation, OperationRequestMap[DocumentMutationOperation]>()

/** Standard canonical request JSON, structurally decoded by the SDK; IR owns meaning. */
export function prepareDocumentMutation(state: DocumentEditorState): PreparedDocumentMutation {
  try {
    let input: Record<string, unknown> = { id: state.id }
    if (state.operation === 'insert' || state.operation === 'upsert') {
      const fields: unknown = JSON.parse(state.fields)
      if (!fields || typeof fields !== 'object' || Array.isArray(fields)) throw new TypeError('Document fields must be a JSON object')
      input = { document: { id: state.id, fields: Object.entries(fields).map(([key, item]) => ({ key, value: toCanonicalJson(value(jsonInput(item))) })) } }
    } else if (state.operation !== 'delete') input.operations = JSON.parse(state.operations)
    if (state.condition !== undefined && state.operation !== 'insert' && state.operation !== 'patch') input.condition = state.condition
    if (state.create !== undefined && state.operation === 'update') input.create = { value: state.create }
    const request = fromCanonicalJson(tables[state.operation], input) as OperationRequestMap[DocumentMutationOperation]
    const prepared = Object.freeze({ operation: state.operation, id: state.id, canonical: JSON.stringify(toCanonicalJson(request), null, 2) })
    requests.set(prepared, request)
    return prepared
  } catch (error) { throw new SearchKernelClientError(error instanceof Error ? error.message : 'Malformed mutation', { code: 'invalid_request', origin: 'client' }) }
}
export function preparedDocumentRequest(prepared: PreparedDocumentMutation) {
  const request = requests.get(prepared)
  if (!request) throw new SearchKernelClientError('Mutation must be prepared by the Search Kernel adapter', { code: 'invalid_request', origin: 'client' })
  return request
}

/** Editing helpers produce existing canonical operation JSON, never a new language. */
export function mutationOperation(operation: 'patch' | 'update', field: string, kind: string, input: string): unknown {
  const item = () => toCanonicalJson(value(jsonInput(JSON.parse(input))))
  const payload = operation === 'patch'
    ? kind === 'clear' ? { operation_type: 'UnsetField', operation: { field } } : { operation_type: 'SetField', operation: { field, value: item() } }
    : { target: { path: field }, ...(kind === 'assign' ? { operation_type: 'AssignUpdate', operation: { value: item() } }
      : kind === 'clear' ? { operation_type: 'ClearUpdate', operation: {} }
      : { operation_type: 'ArithmeticUpdate', operation: { operator: kind[0].toUpperCase() + kind.slice(1), operand: item() } }) }
  const table = operation === 'patch' ? 'PatchOperation' : 'UpdateOperation'
  return toCanonicalJson(fromCanonicalJson(table, payload))
}
