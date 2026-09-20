import {
  connect as connectHttp2,
  constants as http2Constants,
  type ClientHttp2Session,
  type ClientHttp2Stream,
  type IncomingHttpHeaders,
} from "node:http2";
import { connect as connectNet } from "node:net";
import { FlatBufferFrame } from "../generated/grpc/rpc/searchkernel_pb.js";
import { TransportError, type ExecuteOptions } from "./grpc-web.js";

export const MAX_GRPC_MESSAGE_SIZE = 256 * 1024 * 1024;
export const DEFAULT_GRPC_TIMEOUT_MS = 60_000;

const EXECUTE_PATH = "/searchkernel.rpc.v1.SearchKernel/Execute";

function grpcTarget(endpoint: string): {
  authority: string;
  createConnection?: () => ReturnType<typeof connectNet>;
} {
  if (endpoint.startsWith("unix://")) {
    const socketPath = endpoint.slice("unix://".length);
    return {
      authority: "http://localhost",
      createConnection: () => connectNet(socketPath),
    };
  }
  if (endpoint.startsWith("unix:")) {
    const socketPath = endpoint.slice("unix:".length);
    return {
      authority: "http://localhost",
      createConnection: () => connectNet(socketPath),
    };
  }
  return {
    authority: endpoint.includes("://") ? endpoint : `http://${endpoint}`,
  };
}

function grpcFrame(message: Uint8Array): Buffer {
  if (message.byteLength > MAX_GRPC_MESSAGE_SIZE) {
    throw new TransportError("gRPC request exceeds maximum message size");
  }
  const framed = Buffer.allocUnsafe(5 + message.byteLength);
  framed[0] = 0;
  framed.writeUInt32BE(message.byteLength, 1);
  Buffer.from(message).copy(framed, 5);
  return framed;
}

function grpcStatus(headers: IncomingHttpHeaders | undefined): number | null {
  const raw = headers?.["grpc-status"];
  if (raw === undefined) return null;
  const text = Array.isArray(raw) ? raw[0] : raw;
  const parsed = Number(text);
  return Number.isInteger(parsed) ? parsed : null;
}


function grpcTimeoutHeader(timeoutMs: number): string {
  const milliseconds = Math.max(1, Math.ceil(timeoutMs));
  if (milliseconds <= 99_999_999) return `${milliseconds}m`;
  const seconds = Math.ceil(milliseconds / 1_000);
  if (seconds <= 99_999_999) return `${seconds}S`;
  const minutes = Math.ceil(seconds / 60);
  if (minutes <= 99_999_999) return `${minutes}M`;
  return `${Math.min(99_999_999, Math.ceil(minutes / 60))}H`;
}

function grpcMessage(headers: IncomingHttpHeaders | undefined): string {
  const raw = headers?.["grpc-message"];
  const text = Array.isArray(raw) ? raw[0] : raw;
  if (typeof text !== "string") return "gRPC request failed";
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

function decodeUnaryMessage(bytes: Buffer): Uint8Array {
  if (bytes.byteLength < 5) throw new TransportError("malformed gRPC response frame");
  if (bytes[0] !== 0) throw new TransportError("compressed gRPC responses are not supported");
  const length = bytes.readUInt32BE(1);
  if (length > MAX_GRPC_MESSAGE_SIZE) {
    throw new TransportError("gRPC response exceeds maximum message size");
  }
  if (bytes.byteLength !== length + 5) {
    throw new TransportError("malformed unary gRPC response frame");
  }
  return bytes.subarray(5);
}

/** Native Node gRPC transport over HTTP/2, including Unix-domain sockets. */
export class NodeGrpcTransport {
  private session: ClientHttp2Session | null = null;
  private closed = false;

  constructor(readonly endpoint: string) {}

  private getSession(): ClientHttp2Session {
    if (this.closed) throw new TransportError("gRPC transport is closed");
    if (this.session && !this.session.closed && !this.session.destroyed) return this.session;

    const target = grpcTarget(this.endpoint);
    const session = connectHttp2(
      target.authority,
      target.createConnection ? { createConnection: target.createConnection } : undefined,
    );
    // A request stream receives actionable connection failures. This listener
    // prevents a session-level error from becoming an unhandled EventEmitter error.
    session.on("error", () => {});
    session.once("close", () => {
      if (this.session === session) this.session = null;
    });
    this.session = session;
    return session;
  }

  execute(payload: Uint8Array, options: ExecuteOptions = {}): Promise<Uint8Array> {
    if (options.signal?.aborted) {
      return Promise.reject(new TransportError("gRPC request aborted"));
    }

    const timeoutMs = options.timeoutMs ?? DEFAULT_GRPC_TIMEOUT_MS;
    if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
      return Promise.reject(new TransportError("timeoutMs must be a non-negative finite number"));
    }

    let session: ClientHttp2Session;
    try {
      session = this.getSession();
    } catch (error) {
      return Promise.reject(error);
    }

    const request = new FlatBufferFrame();
    request.setPayload(payload);
    let body: Buffer;
    try {
      body = grpcFrame(request.serializeBinary());
    } catch (error) {
      return Promise.reject(error);
    }

    return new Promise((resolve, reject) => {
      let settled = false;
      let stream: ClientHttp2Stream | null = null;
      let responseHeaders: IncomingHttpHeaders | undefined;
      let trailers: IncomingHttpHeaders | undefined;
      const chunks: Buffer[] = [];

      const finish = (error?: TransportError, response?: Uint8Array) => {
        if (settled) return;
        settled = true;
        if (timer) clearTimeout(timer);
        options.signal?.removeEventListener("abort", aborted);
        if (error) reject(error);
        else resolve(response!);
      };

      const aborted = () => {
        stream?.close(http2Constants.NGHTTP2_CANCEL);
        finish(new TransportError("gRPC request aborted"));
      };
      options.signal?.addEventListener("abort", aborted, { once: true });

      const timer = setTimeout(() => {
        stream?.close(http2Constants.NGHTTP2_CANCEL);
        finish(new TransportError("gRPC request timed out"));
      }, timeoutMs);

      try {
        stream = session.request({
          ":method": "POST",
          ":path": EXECUTE_PATH,
          "content-type": "application/grpc",
          "te": "trailers",
          "grpc-timeout": grpcTimeoutHeader(timeoutMs),
        });
        stream.on("response", (headers: IncomingHttpHeaders) => { responseHeaders = headers; });
        stream.on("trailers", (headers: IncomingHttpHeaders) => { trailers = headers; });
        stream.on("data", (chunk: Buffer | Uint8Array) => {
          chunks.push(Buffer.from(chunk));
        });
        stream.once("error", (error: Error) => {
          finish(new TransportError("gRPC request failed", error));
        });
        stream.once("end", () => {
          try {
            const statusHeaders = trailers ?? responseHeaders;
            const status = grpcStatus(statusHeaders);
            if (status !== 0) {
              finish(new TransportError(
                `Search Kernel Execute failed (${status ?? "unknown"}): ${grpcMessage(statusHeaders)}`,
              ));
              return;
            }
            const framePayload = decodeUnaryMessage(Buffer.concat(chunks));
            const response = FlatBufferFrame.deserializeBinary(framePayload);
            finish(undefined, response.getPayload_asU8());
          } catch (error) {
            finish(error instanceof TransportError
              ? error
              : new TransportError("malformed gRPC response", error));
          }
        });
        stream.end(body);
      } catch (error) {
        finish(new TransportError("gRPC request failed", error));
      }
    });
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.session?.close();
    this.session = null;
  }
}
