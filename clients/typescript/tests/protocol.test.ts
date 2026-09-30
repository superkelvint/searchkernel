import { Builder, ByteBuffer, type IGeneratedObject } from "flatbuffers";
import { describe, expect, it } from "vitest";
import { DocumentT } from "../src/generated/flatbuffers/search-kernel/v2/document.js";
import { ErrorCode } from "../src/generated/flatbuffers/search-kernel/v2/error-code.js";
import { ErrorT } from "../src/generated/flatbuffers/search-kernel/v2/error.js";
import { GetResultT } from "../src/generated/flatbuffers/search-kernel/v2/get-result.js";
import { InsertRequestT } from "../src/generated/flatbuffers/search-kernel/v2/insert-request.js";
import { InsertResultT } from "../src/generated/flatbuffers/search-kernel/v2/insert-result.js";
import { IndexTarget } from "../src/generated/flatbuffers/search-kernel/v2/index-target.js";
import { RequestEnvelope } from "../src/generated/flatbuffers/search-kernel/protocol/v2/request-envelope.js";
import { ResponseEnvelope, ResponseEnvelopeT } from "../src/generated/flatbuffers/search-kernel/protocol/v2/response-envelope.js";
import { ResponsePayload } from "../src/generated/flatbuffers/search-kernel/protocol/v2/response-payload.js";
import {
  packObject,
  packRequest,
  PROTOCOL_VERSION,
  unpackResponse,
  ProtocolError,
} from "../src/protocol/envelope.js";

function response(
  payload: IGeneratedObject = new InsertResultT(),
  version = PROTOCOL_VERSION,
  tag = ResponsePayload.SearchKernel_V2_InsertResult,
): Uint8Array {
  return packObject(new ResponseEnvelopeT(version, tag, payload as never), "SKRS");
}

describe("V2 protocol envelopes", () => {
  it("packs a canonical SKRQ request envelope", () => {
    const bytes = packRequest("insert", new InsertRequestT(new DocumentT("d", [])));
    expect(RequestEnvelope.bufferHasIdentifier(new ByteBuffer(bytes))).toBe(true);
  });

  it("routes a request to its canonical daemon target", () => {
    const bytes = packRequest("insert", new InsertRequestT(new DocumentT("d", [])), "catalog-a");
    const envelope = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(bytes));
    expect(envelope.target(new IndexTarget())?.name()).toBe("catalog-a");
  });

  it("verifies SKRS, version and discriminator", () => {
    expect(ResponseEnvelope.bufferHasIdentifier(new ByteBuffer(response()))).toBe(true);
    expect(unpackResponse(response(), "insert").payloadType).toBe(
      ResponsePayload.SearchKernel_V2_InsertResult,
    );
    expect(() => unpackResponse(response(new InsertResultT(), 1), "insert")).toThrow(/version/);
    expect(() => unpackResponse(response(
      new InsertResultT(),
      PROTOCOL_VERSION,
      ResponsePayload.SearchKernel_V2_DeleteResult,
    ), "insert")).toThrow(/unexpected/);
  });

  it("accepts the canonical Error discriminator", () => {
    const decoded = unpackResponse(
      response(
        new ErrorT(ErrorCode.Unsupported, "nope", [], false),
        PROTOCOL_VERSION,
        ResponsePayload.SearchKernel_V2_Error,
      ),
      "insert",
    );
    expect(decoded.payloadType).toBe(ResponsePayload.SearchKernel_V2_Error);
    expect((decoded.payload as ErrorT).code).toBe(ErrorCode.Unsupported);
  });

  it("accepts nested tables whose vtable follows the table", () => {
    const decoded = unpackResponse(
      response(
        new GetResultT(new DocumentT("d", [])),
        PROTOCOL_VERSION,
        ResponsePayload.SearchKernel_V2_GetResult,
      ),
      "get",
    );
    expect((decoded.payload as GetResultT).document?.id).toBe("d");
  });

  it("rejects wrong-root and malformed responses deterministically", () => {
    const builder = new Builder();
    builder.finish(new InsertResultT().pack(builder));
    expect(() => unpackResponse(builder.asUint8Array(), "insert")).toThrow(/envelope/);
    expect(() => unpackResponse(new Uint8Array([1, 2, 3]), "insert")).toThrow(/malformed|envelope/);
  });

  it("classifies truncated nested payloads as protocol failures", () => {
    const valid = response(
      new ErrorT(ErrorCode.Unsupported, "nested payload", [], false),
      PROTOCOL_VERSION,
      ResponsePayload.SearchKernel_V2_Error,
    );
    const truncated = valid.slice(0, Math.max(8, valid.length - 8));
    expect(() => unpackResponse(truncated, "insert")).toThrow(ProtocolError);
  });

});
