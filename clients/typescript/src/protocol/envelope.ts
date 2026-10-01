import { Builder, ByteBuffer, Encoding, type IGeneratedObject } from "flatbuffers";
import { RequestEnvelope, RequestEnvelopeT } from "../generated/flatbuffers/search-kernel/protocol/v2/request-envelope.js";
import { RequestPayload } from "../generated/flatbuffers/search-kernel/protocol/v2/request-payload.js";
import { ResponseEnvelope } from "../generated/flatbuffers/search-kernel/protocol/v2/response-envelope.js";
import { ResponsePayload } from "../generated/flatbuffers/search-kernel/protocol/v2/response-payload.js";
import { AnalyzeRequestT } from "../generated/flatbuffers/search-kernel/v2/analyze-request.js";
import { DeleteRequestT } from "../generated/flatbuffers/search-kernel/v2/delete-request.js";
import { DescribeRequestT } from "../generated/flatbuffers/search-kernel/v2/describe-request.js";
import { GetRequestT } from "../generated/flatbuffers/search-kernel/v2/get-request.js";
import { InsertRequestT } from "../generated/flatbuffers/search-kernel/v2/insert-request.js";
import { InspectQueryRequestT } from "../generated/flatbuffers/search-kernel/v2/inspect-query-request.js";
import { IndexTargetT } from "../generated/flatbuffers/search-kernel/v2/index-target.js";
import { PatchRequestT } from "../generated/flatbuffers/search-kernel/v2/patch-request.js";
import { SearchRequestT } from "../generated/flatbuffers/search-kernel/v2/search-request.js";
import { UpdateRequestT } from "../generated/flatbuffers/search-kernel/v2/update-request.js";
import { UpsertRequestT } from "../generated/flatbuffers/search-kernel/v2/upsert-request.js";
import { CreateIndexRequestT } from "../generated/flatbuffers/search-kernel/v2/create-index-request.js";
import { OpenIndexRequestT } from "../generated/flatbuffers/search-kernel/v2/open-index-request.js";
import { DeleteIndexRequestT } from "../generated/flatbuffers/search-kernel/v2/delete-index-request.js";

export const PROTOCOL_VERSION = 2;
export const REQUEST_IDENTIFIER = "SKRQ";
export const RESPONSE_IDENTIFIER = "SKRS";

export type Operation = "insert" | "upsert" | "patch" | "update" | "delete" | "get" | "search" | "analyze" | "describe" | "inspectQuery" | "createIndex" | "openIndex" | "deleteIndex";

export interface OperationRequestMap {
  insert: InsertRequestT;
  upsert: UpsertRequestT;
  patch: PatchRequestT;
  update: UpdateRequestT;
  delete: DeleteRequestT;
  get: GetRequestT;
  search: SearchRequestT;
  analyze: AnalyzeRequestT;
  describe: DescribeRequestT;
  inspectQuery: InspectQueryRequestT;
  createIndex: CreateIndexRequestT;
  openIndex: OpenIndexRequestT;
  deleteIndex: DeleteIndexRequestT;
}

const requestTypes: Record<Operation, RequestPayload> = {
  insert: RequestPayload.SearchKernel_V2_InsertRequest,
  upsert: RequestPayload.SearchKernel_V2_UpsertRequest,
  patch: RequestPayload.SearchKernel_V2_PatchRequest,
  update: RequestPayload.SearchKernel_V2_UpdateRequest,
  delete: RequestPayload.SearchKernel_V2_DeleteRequest,
  get: RequestPayload.SearchKernel_V2_GetRequest,
  search: RequestPayload.SearchKernel_V2_SearchRequest,
  analyze: RequestPayload.SearchKernel_V2_AnalyzeRequest,
  describe: RequestPayload.SearchKernel_V2_DescribeRequest,
  inspectQuery: RequestPayload.SearchKernel_V2_InspectQueryRequest,
  createIndex: RequestPayload.SearchKernel_V2_CreateIndexRequest,
  openIndex: RequestPayload.SearchKernel_V2_OpenIndexRequest,
  deleteIndex: RequestPayload.SearchKernel_V2_DeleteIndexRequest,
};

const responseTypes: Record<Operation, ResponsePayload> = {
  insert: ResponsePayload.SearchKernel_V2_InsertResult,
  upsert: ResponsePayload.SearchKernel_V2_UpsertResult,
  patch: ResponsePayload.SearchKernel_V2_PatchResult,
  update: ResponsePayload.SearchKernel_V2_UpdateResult,
  delete: ResponsePayload.SearchKernel_V2_DeleteResult,
  get: ResponsePayload.SearchKernel_V2_GetResult,
  search: ResponsePayload.SearchKernel_V2_SearchResult,
  analyze: ResponsePayload.SearchKernel_V2_AnalyzeResult,
  describe: ResponsePayload.SearchKernel_V2_DescribeResult,
  inspectQuery: ResponsePayload.SearchKernel_V2_InspectQueryResult,
  createIndex: ResponsePayload.SearchKernel_V2_CreateIndexResult,
  openIndex: ResponsePayload.SearchKernel_V2_OpenIndexResult,
  deleteIndex: ResponsePayload.SearchKernel_V2_DeleteIndexResult,
};

export class ProtocolError extends Error { override name = "ProtocolError"; }

export function packObject(object: IGeneratedObject, identifier?: string): Uint8Array {
  const builder = new Builder(1024);
  const offset = object.pack(builder);
  builder.finish(offset, identifier);
  return builder.asUint8Array();
}

export function packRequest<K extends Operation>(
  operation: K,
  payload: OperationRequestMap[K],
  target?: string,
  expectedIndexGeneration?: string,
): Uint8Array {
  let indexTarget: IndexTargetT | null = null;
  if (target !== undefined) {
    indexTarget = new IndexTargetT(target, expectedIndexGeneration ?? null);
  }
  const envelope = new RequestEnvelopeT(
    PROTOCOL_VERSION,
    requestTypes[operation],
    payload as never,
    indexTarget,
  );
  return packObject(envelope, REQUEST_IDENTIFIER);
}

export interface DecodedResponse { payloadType: ResponsePayload; payload: unknown; }

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
      throw new ProtocolError("response is not a Search Kernel V2 envelope");
    }

    const envelope = ResponseEnvelope.getRootAsResponseEnvelope(bb);
    if (envelope.protocolVersion() !== PROTOCOL_VERSION) {
      throw new ProtocolError(`unsupported Search Kernel protocol version: ${envelope.protocolVersion()}`);
    }

    const type = envelope.payloadType();
    const expected = responseTypes[operation];
    if (type !== expected && type !== ResponsePayload.SearchKernel_V2_Error) {
      throw new ProtocolError(`unexpected response payload type ${type} for ${operation}`);
    }

    const unpacked = envelope.unpack();
    if (unpacked.payload === null) {
      throw new ProtocolError("response envelope has no payload");
    }
    return { payloadType: type, payload: unpacked.payload };
  } catch (error) {
    if (error instanceof ProtocolError) throw error;
    throw new ProtocolError("malformed Search Kernel response envelope");
  }
}
