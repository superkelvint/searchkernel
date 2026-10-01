import { Builder, ByteBuffer, type IGeneratedObject } from "flatbuffers";
import { describe, expect, it } from "vitest";
import { DocumentT } from "../src/semantic.js";
import { DocumentT as WireDocumentT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/document.js";
import { ErrorCode } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/error-code.js";
import { ErrorT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/error.js";
import { GetResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/get-result.js";
import { InsertRequestT } from "../src/semantic.js";
import { InsertResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/insert-result.js";
import { IndexTarget } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/index-target.js";
import { RequestEnvelope } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/request-envelope.js";
import { ResponseEnvelope, ResponseEnvelopeT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/response-envelope.js";
import { ResponsePayload } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/response-payload.js";
import {
  packObject,
  packRequest,
  PROTOCOL_VERSION,
  unpackResponse,
  ProtocolError,
} from "../src/_wire/v1/codec.js";

function response(
  payload: IGeneratedObject = new InsertResultT(),
  version = PROTOCOL_VERSION,
  tag = ResponsePayload.SearchKernel_Wire_InsertResult,
): Uint8Array {
  return packObject(new ResponseEnvelopeT(version, tag, payload as never), "SKRS");
}

describe("wire codec envelopes", () => {
  it("selects the first released protocol major", () => {
    expect(PROTOCOL_VERSION).toBe(1);
  });

  it("packs a canonical SKRQ request envelope", () => {
    const bytes = packRequest("insert", new InsertRequestT(new DocumentT("d", [])));
    expect(RequestEnvelope.bufferHasIdentifier(new ByteBuffer(bytes))).toBe(true);
  });

  it("packs logical index selection into the canonical request envelope", () => {
    const bytes = packRequest("insert", new InsertRequestT(new DocumentT("d", [])), "products");
    const envelope = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(bytes));
    expect(envelope.target(new IndexTarget())?.name()).toBe("products");
  });

  it("encodes domain-invalid targets for canonical server validation", () => {
    const bytes = packRequest("insert", new InsertRequestT(new DocumentT("d", [])), "Products");
    const envelope = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(bytes));
    expect(envelope.target(new IndexTarget())?.name()).toBe("Products");
  });

  it("verifies SKRS, version and discriminator", () => {
    expect(ResponseEnvelope.bufferHasIdentifier(new ByteBuffer(response()))).toBe(true);
    expect(unpackResponse(response(), "insert").isError).toBe(false);
    expect(() => unpackResponse(response(new InsertResultT(), 2), "insert")).toThrow(/version/);
    expect(() => unpackResponse(response(
      new InsertResultT(),
      PROTOCOL_VERSION,
      ResponsePayload.SearchKernel_Wire_DeleteResult,
    ), "insert")).toThrow(/unexpected/);
  });

  it("accepts the canonical Error discriminator", () => {
    const decoded = unpackResponse(
      response(
        new ErrorT(ErrorCode.Unsupported, "nope", [], false),
        PROTOCOL_VERSION,
        ResponsePayload.SearchKernel_Wire_Error,
      ),
      "insert",
    );
    expect(decoded.isError).toBe(true);
    expect((decoded.payload as ErrorT).code).toBe(ErrorCode.Unsupported);
  });

  it("accepts nested tables whose vtable follows the table", () => {
    const decoded = unpackResponse(
      response(
        new GetResultT(new WireDocumentT("d", [])),
        PROTOCOL_VERSION,
        ResponsePayload.SearchKernel_Wire_GetResult,
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
      ResponsePayload.SearchKernel_Wire_Error,
    );
    const truncated = valid.slice(0, Math.max(8, valid.length - 8));
    expect(() => unpackResponse(truncated, "insert")).toThrow(ProtocolError);
  });

});
