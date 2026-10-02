import { expect, it } from "vitest";
import { Index } from "../src/client/index.js";
import { value } from "../src/client/values.js";
import { DocumentT } from "../src/semantic.js";
import { GetResultT } from "../src/semantic.js";
import { ValueEntryT } from "../src/semantic.js";
import type { GrpcWebTransport } from "../src/transport/grpc-web.js";

function indexWith(fields: Array<ValueEntryT | null>): Index {
  const index = Index.connect("unused", "test", { execute: async () => new Uint8Array() } as unknown as GrpcWebTransport);
  index.client.get = async () => new GetResultT(new DocumentT("doc:1", fields as ValueEntryT[]));
  return index;
}

it("rejects duplicate GET document field keys", async () => {
  await expect(indexWith([
    new ValueEntryT("same", value(1)),
    new ValueEntryT("same", value(2)),
  ]).get("doc:1")).rejects.toThrow(/duplicate/);
});

it("rejects null GET document field entries", async () => {
  await expect(indexWith([null]).get("doc:1")).rejects.toThrow(/malformed|entry/);
});

it("rejects GET document fields without keys", async () => {
  await expect(indexWith([new ValueEntryT(null, value(1))]).get("doc:1")).rejects.toThrow(/key/);
});

it("rejects GET document fields without Value payloads", async () => {
  await expect(indexWith([new ValueEntryT("missing", null)]).get("doc:1")).rejects.toThrow(/payload|value/i);
});

it("preserves explicit canonical null and valid fields", async () => {
  await expect(indexWith([
    new ValueEntryT("nothing", value(null)),
    new ValueEntryT("count", value(3)),
  ]).get("doc:1")).resolves.toEqual({ _id: "doc:1", nothing: null, count: 3n });
});
