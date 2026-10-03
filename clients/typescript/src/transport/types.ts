export interface ExecuteOptions {
  signal?: AbortSignal;
  timeoutMs?: number;
}

export class TransportError extends Error {
  override name = "TransportError";

  constructor(message: string, readonly cause?: unknown, readonly kind: 'failure' | 'aborted' | 'timeout' = 'failure') {
    super(message);
  }
}
