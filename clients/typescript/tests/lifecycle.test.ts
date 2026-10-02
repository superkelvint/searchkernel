import { ByteBuffer } from "flatbuffers";
import { describe, expect, it } from "vitest";
import { Daemon, Index, type ClientTransport } from "../src/client/index.js";
import { ListIndexesResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/list-indexes-result.js";
import { CreateIndexRequest } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/create-index-request.js";
import { CreateIndexResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/create-index-result.js";
import { DeleteIndexRequest } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/delete-index-request.js";
import { DeleteIndexResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/delete-index-result.js";
import { DescribeResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/describe-result.js";
import { ErrorCode } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/error-code.js";
import { ErrorT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/error.js";
import { IndexDefinitionT } from "../src/semantic.js";
import { IndexDefinitionT as WireIndexDefinitionT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/index-definition.js";
import { IndexTarget } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/index-target.js";
import { OpenIndexRequest } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/open-index-request.js";
import { OpenIndexResultT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/open-index-result.js";
import { RequestEnvelope } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/request-envelope.js";
import { RequestPayload } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/request-payload.js";
import { ResponseEnvelopeT } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/response-envelope.js";
import { ResponsePayload } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/response-payload.js";
import { packObject, PROTOCOL_VERSION } from "../src/_wire/v1/codec.js";

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
  it("discovers targets without a target or opening every index", async () => {
    const requests: Uint8Array[] = [];
    const daemon = new Daemon("unused", { execute: async bytes => {
      requests.push(bytes);
      return envelope(ResponsePayload.SearchKernel_Wire_ListIndexesResult, new ListIndexesResultT(["alpha", "beta"]));
    } });
    expect(await daemon.listIndexes()).toEqual(["alpha", "beta"]);
    expect(requests).toHaveLength(1);
    const request = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[0]!));
    expect(request.target()).toBeNull();
    expect(request.payloadType()).toBe(RequestPayload.SearchKernel_Wire_ListIndexesRequest);
  });

  it("retains open definition and packs selected summary without inventing defaults", async () => {
    const index = await Index.open("unused", "alpha", successfulTransport([
      envelope(ResponsePayload.SearchKernel_Wire_OpenIndexResult, new OpenIndexResultT(new WireIndexDefinitionT("schema"), "generation")),
    ]));
    expect(index.definition.name).toBe("schema");
    expect(index.browse().build().ranking).toBeNull();
    expect(index.browse().build().presentation).toBeNull();
    expect(index.search("orchid").ranking("custom").summary("compact").limit(3).build()).toMatchObject({
      ranking: { profile: "custom" }, presentation: { summary: "compact" }, limit: { value: 3 },
    });
    expect(index.search("orchid").summary("compact").fields("title").build().presentation).toMatchObject({
      summary: "compact", fields: { fields: ["title"] },
    });
  });
  it("targets create and every ordinary operation through the shared request packer", async () => {
    const requests: Uint8Array[] = [];
    const transport: ClientTransport = {
      execute: async (request) => {
        requests.push(request);
        const decoded = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(request));
        return decoded.payloadType() === RequestPayload.SearchKernel_Wire_CreateIndexRequest
          ? envelope(ResponsePayload.SearchKernel_Wire_CreateIndexResult, new CreateIndexResultT(new WireIndexDefinitionT(), "fixture-generation"))
          : envelope(ResponsePayload.SearchKernel_Wire_DescribeResult, new DescribeResultT(new WireIndexDefinitionT()));
      },
      close() {},
    };
    const index = await Index.create("unused", "products", new IndexDefinitionT(), transport);
    await index.describe();

    const lifecycle = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[0]!));
    expect(lifecycle.target(new IndexTarget())?.name()).toBe("products");
    expect(lifecycle.payloadType()).toBe(RequestPayload.SearchKernel_Wire_CreateIndexRequest);
    expect(lifecycle.payload(new CreateIndexRequest())?.unpack()).toBeDefined();
    const ordinary = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[1]!));
    expect(ordinary.target(new IndexTarget())?.name()).toBe("products");
    expect(ordinary.target(new IndexTarget())?.generation()).toBe("fixture-generation");
  });

  it("opens through the canonical request and preserves portable lifecycle errors", async () => {
    let openRequest: Uint8Array | undefined;
    const openTransport: ClientTransport = {
      execute: async (request) => {
        openRequest = request;
        return envelope(
          ResponsePayload.SearchKernel_Wire_OpenIndexResult,
          new OpenIndexResultT(new WireIndexDefinitionT(), "fixture-generation"),
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
    expect(decodedOpen.payloadType()).toBe(RequestPayload.SearchKernel_Wire_OpenIndexRequest);
    expect(decodedOpen.target(new IndexTarget())?.name()).toBe("products");
    expect(decodedOpen.payload(new OpenIndexRequest())?.unpack()).toBeDefined();

    const miss = envelope(
      ResponsePayload.SearchKernel_Wire_Error,
      new ErrorT(ErrorCode.NotFound, "missing", [], false),
    );
    await expect(Index.open("unused", "missing", successfulTransport([miss])))
      .rejects.toMatchObject({ name: "SearchKernelError", code: "not_found" });

    const collision = envelope(
      ResponsePayload.SearchKernel_Wire_Error,
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
          ResponsePayload.SearchKernel_Wire_DeleteIndexResult,
          new DeleteIndexResultT(),
        );
      },
      close: () => { closed += 1; },
    };

    await expect(Index.delete("unused", "products", transport)).resolves.toBeUndefined();

    const decoded = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(deleteRequest!));
    expect(decoded.payloadType()).toBe(RequestPayload.SearchKernel_Wire_DeleteIndexRequest);
    expect(decoded.target(new IndexTarget())?.name()).toBe("products");
    expect(decoded.payload(new DeleteIndexRequest())?.unpack()).toBeDefined();
    expect(closed).toBe(1);
  });

  it("propagates canonical delete errors and still closes its temporary handle", async () => {
    let closed = 0;
    const missing = envelope(
      ResponsePayload.SearchKernel_Wire_Error,
      new ErrorT(ErrorCode.NotFound, "missing", [], false),
    );
    await expect(Index.delete("unused", "missing", {
      execute: async () => missing,
      close: () => { closed += 1; },
    })).rejects.toMatchObject({ name: "SearchKernelError", code: "not_found" });
    expect(closed).toBe(1);
  });

  it("connect binds the current generation before its first ordinary operation", async () => {
    const requests: Uint8Array[] = [];
    const transport: ClientTransport = {
      execute: async (request) => {
        requests.push(request);
        const decoded = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(request));
        return decoded.payloadType() === RequestPayload.SearchKernel_Wire_OpenIndexRequest
          ? envelope(
              ResponsePayload.SearchKernel_Wire_OpenIndexResult,
              new OpenIndexResultT(new WireIndexDefinitionT(), "connected-generation"),
            )
          : envelope(
              ResponsePayload.SearchKernel_Wire_DescribeResult,
              new DescribeResultT(new WireIndexDefinitionT()),
            );
      },
      close() {},
    };

    const index = Index.connect("unused", "products", transport);
    await index.describe();

    expect(requests).toHaveLength(2);
    const open = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[0]!));
    expect(open.payloadType()).toBe(RequestPayload.SearchKernel_Wire_OpenIndexRequest);
    expect(open.target(new IndexTarget())?.name()).toBe("products");

    const ordinary = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(requests[1]!));
    expect(ordinary.target(new IndexTarget())?.name()).toBe("products");
    expect(ordinary.target(new IndexTarget())?.generation()).toBe("connected-generation");
    await index.close();
  });

  it("closing one handle closes only its transport and leaves another handle usable", async () => {
    let firstClosed = 0;
    let secondClosed = 0;
    const responseFor = (request: Uint8Array) => {
      const decoded = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(request));
      return decoded.payloadType() === RequestPayload.SearchKernel_Wire_OpenIndexRequest
        ? envelope(
            ResponsePayload.SearchKernel_Wire_OpenIndexResult,
            new OpenIndexResultT(new WireIndexDefinitionT(), "fixture-generation"),
          )
        : envelope(
            ResponsePayload.SearchKernel_Wire_DescribeResult,
            new DescribeResultT(new WireIndexDefinitionT()),
          );
    };
    const first = Index.connect("unused", "products", {
      execute: async (request) => responseFor(request),
      close: () => { firstClosed += 1; },
    });
    const second = Index.connect("unused", "products", {
      execute: async (request) => responseFor(request),
      close: () => { secondClosed += 1; },
    });

    await first.describe();
    await first.close();
    await second.describe();
    expect(firstClosed).toBe(1);
    expect(secondClosed).toBe(0);
    await second.close();
    expect(secondClosed).toBe(1);
  });
});
