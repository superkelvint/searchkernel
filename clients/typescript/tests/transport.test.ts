import { describe, expect, it } from "vitest";
import { FlatBufferFrame } from "../src/generated/grpc/rpc/searchkernel_pb.js";
import { GrpcWebTransport, TransportError, type GeneratedRpcClient } from "../src/transport/grpc-web.js";
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
