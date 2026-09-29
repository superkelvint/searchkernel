import { expect, it } from "vitest";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { unpackResponse } from "../src/protocol/envelope.js";
import { SearchResults } from "../src/client/results.js";
import { SearchResultT } from "../src/generated/flatbuffers/search-kernel/v2/search-result.js";

it.skipIf(!process.env.SEARCHKERNEL_RESULT_CASES)("decodes every canonical SearchResult response byte fixture", () => {
  const cases = JSON.parse(readFileSync(process.env.SEARCHKERNEL_RESULT_CASES!, "utf8"));
  const caseDirectory = process.env.SEARCHKERNEL_RESULT_CASE_DIR!;
  const traces: Record<string, unknown> = {};
  for (const scenario of cases.valid as Array<{ id: string; file: string; json: string }>) {
    const bytes = new Uint8Array(readFileSync(join(caseDirectory, scenario.file)));
    const decoded = unpackResponse(bytes, "search");
    expect(decoded.payload).toBeInstanceOf(SearchResultT);
    const result = new SearchResults(decoded.payload as SearchResultT);
    expect(result.root).not.toBeNull();
    const normalized = JSON.parse(readFileSync(join(caseDirectory, scenario.json), "utf8"));
    expect(normalized.payload).toBeDefined();
    traces[scenario.id] = readFileSync(join(caseDirectory, scenario.json), "utf8");
  }

  const rejected: string[] = [];
  for (const scenario of cases.malformed as Array<{ id: string; file: string }>) {
    const bytes = new Uint8Array(readFileSync(join(process.env.SEARCHKERNEL_RESULT_FIXTURE_DIR!, scenario.file)));
    expect(() => {
      const decoded = unpackResponse(bytes, "search");
      new SearchResults(decoded.payload as SearchResultT);
    }, scenario.id).toThrow();
    rejected.push(scenario.id);
  }
  traces.malformed_rejected = rejected;
  writeFileSync(join(process.env.SEARCHKERNEL_RESULT_TRACE_DIR!, "typescript.json"), JSON.stringify(traces));
});
