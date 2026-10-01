import { it, expect } from "vitest";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Index } from "../src/client/index.js";
import { float64, int64, uint64 } from "../src/client/values.js";
import { packObject } from "../src/_wire/v1/codec.js";
import type { GrpcWebTransport } from "../src/transport/grpc-web.js";
const parityEnabled =
  Boolean(process.env.SEARCHKERNEL_PARITY_CORPUS) &&
  Boolean(process.env.SEARCHKERNEL_PARITY_OUTPUT);

function corpusValue(value: any): unknown {
  if (value.int64 !== undefined) return int64(value.int64);
  if (value.uint64 !== undefined) return uint64(value.uint64);
  if (value.string !== undefined) return value.string;
  if (value.float64 !== undefined) return float64(value.float64);
  throw new Error(`unsupported parity corpus value: ${JSON.stringify(value)}`);
}

it.skipIf(!parityEnabled)("executes frozen cross-language semantic corpus", () => {
  const root=resolve(process.cwd(),"../.."); const corpus=process.env.SEARCHKERNEL_PARITY_CORPUS!; const data=JSON.parse(readFileSync(corpus,"utf8"));
  const binary=process.env.SEARCHKERNEL_SEMANTIC_SNAPSHOT!; const fixture=process.env.SEARCHKERNEL_PARITY_INDEX!; const out:Record<string,unknown>={};
  if (!binary || !fixture) throw new Error("missing parity snapshot/fixture environment");
  for(const scenario of data.scenarios){ const i=scenario.intent; const index=Index.connect("unused","test",{execute:async()=>new Uint8Array()} as unknown as GrpcWebTransport); let b=i.kind==="browse"?index.browse():index.search(i.query);
    if(i.filter){const bindings:Record<string,unknown>={}; for(const [k,v] of Object.entries<any>(i.filter.bindings)) bindings[k]=corpusValue(v); b=b.filter(i.filter.source,bindings as any);}
    if(i.ranking)b=b.ranking(i.ranking); for(const [k,v] of Object.entries<any>(i.inputs??{}))b=b.input(k,corpusValue(v) as any); for(const [k,v] of Object.entries<any>(i.rank_properties??{}))b=b.rankProperty(k,v); if("limit" in i)b=b.limit(i.limit); if("offset" in i)b=b.offset(i.offset);
    const d=mkdtempSync(join(tmpdir(),"sk-parity-")); const request=join(d,"request.bfbs"); writeFileSync(request,packObject(b.build())); out[scenario.id]=JSON.parse(execFileSync(binary,["--index-json",fixture,"--request",request],{cwd:root,encoding:"utf8"})); rmSync(d,{recursive:true,force:true}); }
  for (const rejection of data.facade_rejections ?? []) {
    if (!rejection.clients.includes("typescript")) continue;
    const intent = rejection.intent;
    const index = Index.connect(
      "unused",
      "test",
      { execute: async () => new Uint8Array() } as unknown as GrpcWebTransport,
    );
    const builder = intent.kind === "browse" ? index.browse() : index.search(intent.query);
    for (const [key, value] of Object.entries(intent.rank_properties ?? {})) {
      expect(() => builder.rankProperty(key, value as string)).toThrow(TypeError);
    }
  }
  writeFileSync(process.env.SEARCHKERNEL_PARITY_OUTPUT!,JSON.stringify(out)); expect(Object.keys(out)).toHaveLength(data.scenarios.length);
}, 120_000);
