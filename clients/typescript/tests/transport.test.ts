import { describe, expect, it } from "vitest";
import type { ClientUnaryCall } from "@grpc/grpc-js";
import { FlatBufferFrame } from "../src/generated/grpc/rpc/searchkernel_pb.js";
import {
  GrpcWebTransport,
  TransportError,
  type GeneratedRpcClient,
} from "../src/transport/grpc-web.js";
import { NodeGrpcTransport, type GeneratedNodeRpcClient } from "../src/transport/grpc-node.js";
describe("gRPC-Web byte transport", () => {
  it("passes FlatBuffer bytes through unchanged", async () => {
    let sent: Uint8Array | undefined;
    const client: GeneratedRpcClient = { execute(request, _metadata, done) { sent = request.getPayload_asU8(); const response = new FlatBufferFrame(); response.setPayload(new Uint8Array([9, 8])); queueMicrotask(() => done(null, response)); return { cancel() {} }; } };
    await expect(new GrpcWebTransport("unused", client).execute(new Uint8Array([1, 2]))).resolves.toEqual(new Uint8Array([9, 8]));
    expect(sent).toEqual(new Uint8Array([1, 2]));
  });
  it("keeps transport failures distinct", async () => {
    const client: GeneratedRpcClient = { execute(_r, _m, done) { queueMicrotask(() => done({ message: "down" } as never, new FlatBufferFrame())); return { cancel() {} }; } };
    await expect(new GrpcWebTransport("unused", client).execute(new Uint8Array())).rejects.toBeInstanceOf(TransportError);
  });
  it("cancels an aborted request", async () => {
    let cancelled = false; const controller = new AbortController();
    const client: GeneratedRpcClient = { execute() { return { cancel() { cancelled = true; } }; } };
    const pending = new GrpcWebTransport("unused", client).execute(new Uint8Array(), { signal: controller.signal }); controller.abort();
    await expect(pending).rejects.toThrow(/aborted/); expect(cancelled).toBe(true);
  });
  it("cancels a timed-out request", async () => {
    let cancelled = false;
    const client: GeneratedRpcClient = { execute() { return { cancel() { cancelled = true; } }; } };
    await expect(new GrpcWebTransport("unused", client).execute(new Uint8Array(), { timeoutMs: 1 })).rejects.toThrow(/timed out/);
    expect(cancelled).toBe(true);
  });
});

describe("Node gRPC byte transport", () => {
  it("passes FlatBuffer bytes through the generated client unchanged", async () => {
    let sent: Uint8Array | undefined;
    const client: GeneratedNodeRpcClient = {
      execute(request, _metadata, _options, done) {
        sent = request.getPayload_asU8();
        const response = new FlatBufferFrame();
        response.setPayload(new Uint8Array([9, 8]));
        queueMicrotask(() => done(null, response));
        return { cancel() {}, getPeer() { return "test"; } } as ClientUnaryCall;
      },
      close() {},
    };

    await expect(new NodeGrpcTransport("unused", client).execute(new Uint8Array([1, 2])))
      .resolves.toEqual(new Uint8Array([9, 8]));
    expect(sent).toEqual(new Uint8Array([1, 2]));
  });

  it("cancels generated calls on abort and timeout", async () => {
    let cancelled = 0;
    const client: GeneratedNodeRpcClient = {
      execute() {
        return {
          cancel() { cancelled += 1; },
          getPeer() { return "test"; },
        } as ClientUnaryCall;
      },
      close() {},
    };

    const controller = new AbortController();
    const transport = new NodeGrpcTransport("unused", client);
    const aborted = transport.execute(new Uint8Array(), { signal: controller.signal });
    controller.abort();
    await expect(aborted).rejects.toThrow(/aborted/);

    await expect(transport.execute(new Uint8Array(), { timeoutMs: 1 })).rejects.toThrow(/timed out/);
    expect(cancelled).toBe(2);
  });
});
