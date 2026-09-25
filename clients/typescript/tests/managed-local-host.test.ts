import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";


describe("managed-local native runtime contract", () => {
  it("does not probe the retired monolithic native SDK path", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/node/managed-local-host.ts"),
      "utf8",
    );

    expect(source).not.toContain('"native",\n      "sdk"');
    expect(source).not.toContain("core/native/sdk");
  });
});
