import { Builder, ByteBuffer, Encoding, type IGeneratedObject, type Offset } from "flatbuffers";
import { RequestEnvelope, RequestEnvelopeT } from "./generated/flatbuffers/search-kernel/protocol/request-envelope.js";
import { RequestPayload } from "./generated/flatbuffers/search-kernel/protocol/request-payload.js";
import { ResponseEnvelope } from "./generated/flatbuffers/search-kernel/protocol/response-envelope.js";
import { ResponsePayload } from "./generated/flatbuffers/search-kernel/protocol/response-payload.js";
import { AnalyzeRequestT } from "./generated/flatbuffers/search-kernel/wire/analyze-request.js";
import { DeleteRequestT } from "./generated/flatbuffers/search-kernel/wire/delete-request.js";
import { DescribeRequestT } from "./generated/flatbuffers/search-kernel/wire/describe-request.js";
import { GetRequestT } from "./generated/flatbuffers/search-kernel/wire/get-request.js";
import { InsertRequestT } from "./generated/flatbuffers/search-kernel/wire/insert-request.js";
import { InspectQueryRequestT } from "./generated/flatbuffers/search-kernel/wire/inspect-query-request.js";
import { IndexTargetT } from "./generated/flatbuffers/search-kernel/wire/index-target.js";
import { PatchRequestT } from "./generated/flatbuffers/search-kernel/wire/patch-request.js";
import { SearchRequestT } from "./generated/flatbuffers/search-kernel/wire/search-request.js";
import { UpdateRequestT } from "./generated/flatbuffers/search-kernel/wire/update-request.js";
import { UpsertRequestT } from "./generated/flatbuffers/search-kernel/wire/upsert-request.js";
import { CreateIndexRequestT } from "./generated/flatbuffers/search-kernel/wire/create-index-request.js";
import { OpenIndexRequestT } from "./generated/flatbuffers/search-kernel/wire/open-index-request.js";
import { FilterRequest, FilterRequestT } from "./generated/flatbuffers/search-kernel/wire/filter-request.js";
import type { FilterClauseT as WireFilterClauseT } from "./generated/flatbuffers/search-kernel/wire/filter-clause.js";
import * as Wire from "./generated/flatbuffers/search-kernel/wire.js";
import * as Semantic from "../../semantic.js";
import { SemanticTable, isSemanticTable } from "../../semantic.js";

export const PROTOCOL_VERSION = 1;
export const REQUEST_IDENTIFIER = "SKRQ";
export const RESPONSE_IDENTIFIER = "SKRS";

export type Operation = "insert" | "upsert" | "patch" | "update" | "delete" | "get" | "search" | "analyze" | "describe" | "inspectQuery" | "createIndex" | "openIndex";

export interface OperationRequestMap {
  insert: Semantic.InsertRequestT;
  upsert: Semantic.UpsertRequestT;
  patch: Semantic.PatchRequestT;
  update: Semantic.UpdateRequestT;
  delete: Semantic.DeleteRequestT;
  get: Semantic.GetRequestT;
  search: Semantic.SearchRequestT;
  analyze: Semantic.AnalyzeRequestT;
  describe: Semantic.DescribeRequestT;
  inspectQuery: Semantic.InspectQueryRequestT;
  createIndex: Semantic.CreateIndexRequestT;
  openIndex: Semantic.OpenIndexRequestT;
}

const requestTypes: Record<Operation, RequestPayload> = {
  insert: RequestPayload.SearchKernel_Wire_InsertRequest,
  upsert: RequestPayload.SearchKernel_Wire_UpsertRequest,
  patch: RequestPayload.SearchKernel_Wire_PatchRequest,
  update: RequestPayload.SearchKernel_Wire_UpdateRequest,
  delete: RequestPayload.SearchKernel_Wire_DeleteRequest,
  get: RequestPayload.SearchKernel_Wire_GetRequest,
  search: RequestPayload.SearchKernel_Wire_SearchRequest,
  analyze: RequestPayload.SearchKernel_Wire_AnalyzeRequest,
  describe: RequestPayload.SearchKernel_Wire_DescribeRequest,
  inspectQuery: RequestPayload.SearchKernel_Wire_InspectQueryRequest,
  createIndex: RequestPayload.SearchKernel_Wire_CreateIndexRequest,
  openIndex: RequestPayload.SearchKernel_Wire_OpenIndexRequest,
};

const responseTypes: Record<Operation, ResponsePayload> = {
  insert: ResponsePayload.SearchKernel_Wire_InsertResult,
  upsert: ResponsePayload.SearchKernel_Wire_UpsertResult,
  patch: ResponsePayload.SearchKernel_Wire_PatchResult,
  update: ResponsePayload.SearchKernel_Wire_UpdateResult,
  delete: ResponsePayload.SearchKernel_Wire_DeleteResult,
  get: ResponsePayload.SearchKernel_Wire_GetResult,
  search: ResponsePayload.SearchKernel_Wire_SearchResult,
  analyze: ResponsePayload.SearchKernel_Wire_AnalyzeResult,
  describe: ResponsePayload.SearchKernel_Wire_DescribeResult,
  inspectQuery: ResponsePayload.SearchKernel_Wire_InspectQueryResult,
  createIndex: ResponsePayload.SearchKernel_Wire_CreateIndexResult,
  openIndex: ResponsePayload.SearchKernel_Wire_OpenIndexResult,
};

export class ProtocolError extends Error { override name = "ProtocolError"; }

function toGenerated(value: any): any {
  if (value instanceof Uint8Array) return value;
  if (isSemanticTable(value)) {
    if (value._schemaName === "FilterRequest" && (value as any)._textualOnly === true) {
      return new TextualFilterRequest((value as any).textual.map(toGenerated) as WireFilterClauseT[]);
    }
    const constructor = (Wire as any)[`${value._schemaName}T`];
    if (typeof constructor !== "function") throw new ProtocolError(`unsupported semantic table ${value._schemaName}`);
    const fields = value._semanticFields.map((field) => toGenerated((value as any)[field]));
    return new constructor(...fields);
  }
  if (Array.isArray(value)) return value.map(toGenerated);
  if (value && typeof value === "object") {
    const output: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) output[key] = toGenerated(item);
    return output;
  }
  return value;
}

class TextualFilterRequest extends FilterRequestT {
  constructor(textual: WireFilterClauseT[]) {
    super([], [], [], textual);
  }

  override pack(builder: Builder): Offset {
    const textual = FilterRequest.createTextualVector(
      builder,
      builder.createObjectOffsetList(this.textual),
    );
    FilterRequest.startFilterRequest(builder);
    FilterRequest.addTextual(builder, textual);
    return FilterRequest.endFilterRequest(builder);
  }
}

function fromGenerated(value: any): any {
  if (value instanceof Uint8Array) return value;
  if (Array.isArray(value)) return value.map(fromGenerated);
  if (value && typeof value === "object") {
    const name = value.constructor?.name;
    if (typeof name === "string" && name.endsWith("T") && name !== "RequestEnvelopeT" && name !== "ResponseEnvelopeT") {
      const semanticConstructor = (Semantic as any)[name];
      if (typeof semanticConstructor === "function") {
        const semantic = new semanticConstructor();
        for (const field of semantic._semanticFields) semantic[field] = fromGenerated(value[field]);
        return semantic;
      }
    }
    const output: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) output[key] = fromGenerated(item);
    return output;
  }
  return value;
}

export function packObject(object: SemanticTable | IGeneratedObject, identifier?: string): Uint8Array {
  const builder = new Builder(1024);
  const generated = isSemanticTable(object) ? toGenerated(object) : object;
  const offset = generated.pack(builder);
  builder.finish(offset, identifier);
  return builder.asUint8Array();
}

export function packRequest<K extends Operation>(
  operation: K,
  payload: OperationRequestMap[K],
  target?: string,
): Uint8Array {
  let indexTarget: IndexTargetT | null = null;
  if (target !== undefined) {
    indexTarget = new IndexTargetT(target);
  }
  const envelope = new RequestEnvelopeT(
    PROTOCOL_VERSION,
    requestTypes[operation],
    toGenerated(payload) as never,
    indexTarget,
  );
  return packObject(envelope, REQUEST_IDENTIFIER);
}

export interface DecodedResponse { isError: boolean; payload: unknown; }

export function expectedResponseType(operation: Operation): ResponsePayload { return responseTypes[operation]; }

/**
 * The JavaScript FlatBuffers accessors are intentionally permissive: missing
 * bytes can become `undefined` or an empty subarray instead of an exception.
 * Responses cross a trust boundary, so make reads fail closed while unpacking
 * the envelope and its generated payload object graph.
 */
class CheckedByteBuffer extends ByteBuffer {
  private assertRange(offset: number, length: number): void {
    if (
      !Number.isSafeInteger(offset)
      || !Number.isSafeInteger(length)
      || offset < 0
      || length < 0
      || offset > this.capacity() - length
    ) {
      throw new RangeError("FlatBuffer access exceeds response bounds");
    }
  }

  override readUint8(offset: number): number {
    this.assertRange(offset, 1);
    return super.readUint8(offset);
  }

  override __offset(bbPos: number, vtableOffset: number): number {
    this.assertRange(bbPos, 4);
    const vtable = bbPos - super.readInt32(bbPos);
    this.assertRange(vtable, 4);

    const vtableLength = super.readUint16(vtable);
    const objectLength = super.readUint16(vtable + 2);
    if (vtableLength < 4 || objectLength < 4) {
      throw new RangeError("FlatBuffer table has an invalid header");
    }
    this.assertRange(vtable, vtableLength);
    this.assertRange(bbPos, objectLength);

    const fieldOffset = super.__offset(bbPos, vtableOffset);
    if (fieldOffset && fieldOffset >= objectLength) {
      throw new RangeError("FlatBuffer field is outside its table");
    }
    return fieldOffset;
  }

  override __string(offset: number, optionalEncoding?: Encoding): string | Uint8Array {
    const stringData = offset + super.readInt32(offset);
    const length = super.readInt32(stringData);
    const stringStart = stringData + 4;
    // FlatBuffers strings carry a trailing NUL, which must also be present.
    this.assertRange(stringStart, length + 1);
    return super.__string(offset, optionalEncoding);
  }
}

export function unpackResponse(bytes: Uint8Array, operation: Operation): DecodedResponse {
  try {
    const bb = new CheckedByteBuffer(bytes);
    if (!ResponseEnvelope.bufferHasIdentifier(bb)) {
      throw new ProtocolError("response is not a Search Kernel envelope");
    }

    const envelope = ResponseEnvelope.getRootAsResponseEnvelope(bb);
    if (envelope.protocolVersion() !== PROTOCOL_VERSION) {
      throw new ProtocolError(`unsupported Search Kernel protocol version: ${envelope.protocolVersion()}`);
    }

    const type = envelope.payloadType();
    const expected = responseTypes[operation];
    if (type !== expected && type !== ResponsePayload.SearchKernel_Wire_Error) {
      throw new ProtocolError(`unexpected response payload type ${type} for ${operation}`);
    }

    const unpacked = envelope.unpack();
    if (unpacked.payload === null) {
      throw new ProtocolError("response envelope has no payload");
    }
    return { isError: type === ResponsePayload.SearchKernel_Wire_Error, payload: fromGenerated(unpacked.payload) };
  } catch (error) {
    if (error instanceof ProtocolError) throw error;
    throw new ProtocolError("malformed Search Kernel response envelope");
  }
}
