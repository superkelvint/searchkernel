import {
  credentials,
  type CallOptions,
  type ChannelCredentials,
  type ClientUnaryCall,
  Metadata,
  type ServiceError,
} from "@grpc/grpc-js";
import { FlatBufferFrame } from "../generated/grpc/rpc/searchkernel_pb.js";
import { TransportError, type ExecuteOptions } from "./types.js";

// grpc-tools emits the standard Node stub as JavaScript without a declaration
// file; the implementation and method path remain generated from searchkernel.proto.
const { SearchKernelClient } = require("../generated/grpc/rpc/searchkernel_grpc_pb.js") as {
  SearchKernelClient: new (
    endpoint: string,
    credentials: ChannelCredentials,
    options?: Record<string, unknown>,
  ) => unknown;
};

export const DEFAULT_GRPC_TIMEOUT_MS = 60_000;
export const MAX_GRPC_MESSAGE_SIZE = 256 * 1024 * 1024;

function grpcTarget(endpoint: string): string {
  if (endpoint.startsWith("unix://")) return endpoint;
  if (endpoint.startsWith("unix:")) return `unix://${endpoint.slice("unix:".length)}`;
  return endpoint;
}

export interface GeneratedNodeRpcClient {
  execute(
    request: FlatBufferFrame,
    metadata: unknown,
    options: CallOptions,
    callback: (error: ServiceError | null, response?: FlatBufferFrame) => void,
  ): ClientUnaryCall;
  close(): void;
}

function createClient(endpoint: string): GeneratedNodeRpcClient {
  return new SearchKernelClient(
    grpcTarget(endpoint),
    credentials.createInsecure(),
    {
      "grpc.max_send_message_length": MAX_GRPC_MESSAGE_SIZE,
      "grpc.max_receive_message_length": MAX_GRPC_MESSAGE_SIZE,
    },
  ) as unknown as GeneratedNodeRpcClient;
}

/** Thin Node adapter around the standard generated gRPC client. */
export class NodeGrpcTransport {
  private closed = false;

  constructor(
    readonly endpoint: string,
    private readonly client: GeneratedNodeRpcClient = createClient(endpoint),
  ) {}

  execute(payload: Uint8Array, options: ExecuteOptions = {}): Promise<Uint8Array> {
    if (options.signal?.aborted) {
      return Promise.reject(new TransportError("gRPC request aborted"));
    }
    if (this.closed) {
      return Promise.reject(new TransportError("gRPC transport is closed"));
    }

    const timeoutMs = options.timeoutMs ?? DEFAULT_GRPC_TIMEOUT_MS;
    if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
      return Promise.reject(new TransportError("timeoutMs must be a non-negative finite number"));
    }

    const request = new FlatBufferFrame();
    request.setPayload(payload);

    return new Promise((resolve, reject) => {
      let settled = false;
      let call: ClientUnaryCall | undefined;
      let timer: ReturnType<typeof setTimeout> | undefined;

      const finish = (error?: TransportError, response?: Uint8Array) => {
        if (settled) return;
        settled = true;
        if (timer) clearTimeout(timer);
        options.signal?.removeEventListener("abort", aborted);
        if (error) reject(error);
        else resolve(response!);
      };

      const aborted = () => {
        call?.cancel();
        finish(new TransportError("gRPC request aborted"));
      };
      options.signal?.addEventListener("abort", aborted, { once: true });

      timer = setTimeout(() => {
        call?.cancel();
        finish(new TransportError("gRPC request timed out"));
      }, timeoutMs);

      try {
        const deadline = new Date(Date.now() + timeoutMs);
        call = this.client.execute(
          request,
          new Metadata(),
          { deadline },
          (error, response) => {
            if (error) {
              finish(new TransportError(error.message || "gRPC request failed", error));
              return;
            }
            if (!response) {
              finish(new TransportError("gRPC response was empty"));
              return;
            }
            finish(undefined, response.getPayload_asU8());
          },
        );
      } catch (error) {
        finish(new TransportError("gRPC request failed", error));
      }
    });
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.client.close();
  }
}
