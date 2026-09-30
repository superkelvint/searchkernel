import { ByteBuffer } from "flatbuffers";
import { describe, expect, it } from "vitest";
import { Index, type ClientTransport } from "../src/client/index.js";
import { CreateIndexRequest } from "../src/generated/flatbuffers/search-kernel/v2/create-index-request.js";
import { CreateIndexResultT } from "../src/generated/flatbuffers/search-kernel/v2/create-index-result.js";
import { DeleteIndexRequest } from "../src/generated/flatbuffers/search-kernel/v2/delete-index-request.js";
import { DeleteIndexResultT } from "../src/generated/flatbuffers/search-kernel/v2/delete-index-result.js";
import { DescribeResultT } from "../src/generated/flatbuffers/search-kernel/v2/describe-result.js";
import { ErrorCode } from "../src/generated/flatbuffers/search-kernel/v2/error-code.js";
import { ErrorT } from "../src/generated/flatbuffers/search-kernel/v2/error.js";
import { IndexDefinitionT } from "../src/generated/flatbuffers/search-kernel/v2/index-definition.js";
import { IndexTarget } from "../src/generated/flatbuffers/search-kernel/v2/index-target.js";
import { OpenIndexRequest } from "../src/generated/flatbuffers/search-kernel/v2/open-index-request.js";
import { OpenIndexResultT } from "../src/generated/flatbuffers/search-kernel/v2/open-index-result.js";
import { RequestEnvelope } from "../src/generated/flatbuffers/search-kernel/protocol/v2/request-envelope.js";
import { RequestPayload } from "../src/generated/flatbuffers/search-kernel/protocol/v2/request-payload.js";
import { ResponseEnvelopeT } from "../src/generated/flatbuffers/search-kernel/protocol/v2/response-envelope.js";
import { ResponsePayload } from "../src/generated/flatbuffers/search-kernel/protocol/v2/response-payload.js";
import { packObject, PROTOCOL_VERSION } from "../src/protocol/envelope.js";

function envelope(type: ResponsePayload, payload: object): Uint8Array {
  return packObject(new ResponseEnvelopeT(PROTOCOL_VERSION, type, payload as never), "SKRS");
}

function successfulTransport(responses: Uint8Array[]): ClientTransport {
  return {
    execute: async () => responses.shift() ?? new Uint8Array(),
    close() {},
  };
}

describe("logical index lifecycle", () => {
  it("targets create and every ordinary operation through the shared request packer", async () => {
    const requests: Uint8Array[] = [];
    const transport: ClientTransport = {
      execute: async (request) => {
        requests.push(request);
        const decoded = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(request));
        return decoded.payloadType() === RequestPayload.SearchKernel_V2_CreateIndexRequest
          ? envelope(ResponsePayload.SearchKernel_V2_CreateIndexResult, new CreateIndexResultT(new IndexDefinitionT()))
          : envelope(ResponsePayload.SearchKernel_V2_DescribeResult, new DescribeResultT(new IndexDefinitionT()));
      },
      close() {},
    };
    const index = await Index.create("unused", "products", new IndexDefinitionT(), transport);
    await index.describe();

    const lifecycle = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[0]!));
    expect(lifecycle.target(new IndexTarget())?.name()).toBe("products");
    expect(lifecycle.payloadType()).toBe(RequestPayload.SearchKernel_V2_CreateIndexRequest);
    expect(lifecycle.payload(new CreateIndexRequest())?.unpack()).toBeDefined();
    const ordinary = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[1]!));
    expect(ordinary.target(new IndexTarget())?.name()).toBe("products");
  });

  it("opens through the canonical request and preserves portable lifecycle errors", async () => {
    let openRequest: Uint8Array | undefined;
    const openTransport: ClientTransport = {
      execute: async (request) => {
        openRequest = request;
        return envelope(
          ResponsePayload.SearchKernel_V2_OpenIndexResult,
          new OpenIndexResultT(new IndexDefinitionT()),
        );
      },
      close() {},
    };
    const opened = await Index.open(
      "unused",
      "products",
      openTransport,
    );
    expect(opened.client.target).toBe("products");
    const decodedOpen = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(openRequest!));
    expect(decodedOpen.payloadType()).toBe(RequestPayload.SearchKernel_V2_OpenIndexRequest);
    expect(decodedOpen.target(new IndexTarget())?.name()).toBe("products");
    expect(decodedOpen.payload(new OpenIndexRequest())?.unpack()).toBeDefined();

    const miss = envelope(
      ResponsePayload.SearchKernel_V2_Error,
      new ErrorT(ErrorCode.NotFound, "missing", [], false),
    );
    await expect(Index.open("unused", "missing", successfulTransport([miss])))
      .rejects.toMatchObject({ name: "SearchKernelError", code: "not_found" });

    const collision = envelope(
      ResponsePayload.SearchKernel_V2_Error,
      new ErrorT(ErrorCode.AlreadyExists, "exists", [], false),
    );
    await expect(Index.create("unused", "products", new IndexDefinitionT(), successfulTransport([collision])))
      .rejects.toMatchObject({ name: "SearchKernelError", code: "already_exists" });
  });

  it("deletes through the canonical request and closes its temporary handle", async () => {
    let deleteRequest: Uint8Array | undefined;
    let closed = 0;
    const transport: ClientTransport = {
      execute: async (request) => {
        deleteRequest = request;
        return envelope(
          ResponsePayload.SearchKernel_V2_DeleteIndexResult,
          new DeleteIndexResultT(),
        );
      },
      close: () => { closed += 1; },
    };

    await expect(Index.delete("unused", "products", transport)).resolves.toBeUndefined();

    const decoded = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(deleteRequest!));
    expect(decoded.payloadType()).toBe(RequestPayload.SearchKernel_V2_DeleteIndexRequest);
    expect(decoded.target(new IndexTarget())?.name()).toBe("products");
    expect(decoded.payload(new DeleteIndexRequest())?.unpack()).toBeDefined();
    expect(closed).toBe(1);
  });

  it("propagates canonical delete errors and still closes its temporary handle", async () => {
    let closed = 0;
    const missing = envelope(
      ResponsePayload.SearchKernel_V2_Error,
      new ErrorT(ErrorCode.NotFound, "missing", [], false),
    );
    await expect(Index.delete("unused", "missing", {
      execute: async () => missing,
      close: () => { closed += 1; },
    })).rejects.toMatchObject({ name: "SearchKernelError", code: "not_found" });
    expect(closed).toBe(1);
  });

  it("closing one handle closes only its transport and leaves another handle usable", async () => {
    let firstClosed = 0;
    let secondClosed = 0;
    const first = Index.connect("unused", "products", {
      execute: async () => new Uint8Array(),
      close: () => { firstClosed += 1; },
    });
    const second = Index.connect("unused", "products", {
      execute: async () => envelope(
        ResponsePayload.SearchKernel_V2_DescribeResult,
        new DescribeResultT(new IndexDefinitionT()),
      ),
      close: () => { secondClosed += 1; },
    });
    await first.close();
    await second.describe();
    expect(firstClosed).toBe(1);
    expect(secondClosed).toBe(0);
    await second.close();
    expect(secondClosed).toBe(1);
  });
});
