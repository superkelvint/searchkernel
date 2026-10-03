import type { RpcError } from "grpc-web";
import { FlatBufferFrame } from "../generated/grpc/rpc/searchkernel_pb.js";
import { SearchKernelClient } from "../generated/grpc/rpc/SearchkernelServiceClientPb.js";
import { TransportError, type ExecuteOptions } from "./types.js";

export interface Cancelable { cancel(): void; }
export interface GeneratedRpcClient {
  execute(request: FlatBufferFrame, metadata: Record<string, string> | null,
    callback: (error: RpcError | null, response: FlatBufferFrame) => void): Cancelable;
}
export { TransportError } from "./types.js";
export type { ExecuteOptions } from "./types.js";

/** Deliberately opaque gRPC-Web transport: FlatBuffer bytes in, bytes out. */
export class GrpcWebTransport {
  readonly client: GeneratedRpcClient;
  constructor(endpoint: string, client: GeneratedRpcClient = new SearchKernelClient(endpoint) as unknown as GeneratedRpcClient) { this.client = client; }
  execute(payload: Uint8Array, options: ExecuteOptions = {}): Promise<Uint8Array> {
    if (options.signal?.aborted) return Promise.reject(new TransportError("gRPC-Web request aborted", undefined, 'aborted'));
    const request = new FlatBufferFrame(); request.setPayload(payload);
    return new Promise((resolve, reject) => {
      let settled = false;
      let timer: ReturnType<typeof setTimeout> | undefined;
      let call: Cancelable | undefined;
      const fail = (error: TransportError) => { if (!settled) { settled = true; call?.cancel(); if (timer) clearTimeout(timer); options.signal?.removeEventListener("abort", aborted); reject(error); } };
      const aborted = () => fail(new TransportError("gRPC-Web request aborted", undefined, 'aborted'));
      options.signal?.addEventListener("abort", aborted, { once: true });
      if (options.timeoutMs !== undefined) {
        if (!Number.isFinite(options.timeoutMs) || options.timeoutMs < 0) return fail(new TransportError("timeoutMs must be a non-negative finite number"));
        timer = setTimeout(() => fail(new TransportError("gRPC-Web request timed out", undefined, 'timeout')), options.timeoutMs);
      }
      try {
        call = this.client.execute(request, null, (error, response) => {
          if (settled) return;
          settled = true;
          if (timer) clearTimeout(timer);
          options.signal?.removeEventListener("abort", aborted);
          if (error) { reject(new TransportError(error.message || "gRPC-Web request failed", error)); return; }
          resolve(response.getPayload_asU8());
        });
      } catch (error) { fail(new TransportError("gRPC-Web request failed", error)); }
    });
  }
}
