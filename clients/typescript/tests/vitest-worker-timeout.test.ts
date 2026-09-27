import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);

describe("Vitest worker RPC regression", () => {
  it("uses a Vitest major with the slow onTaskUpdate worker fix", () => {
    const packageJsonPath = require.resolve("vitest/package.json");
    const version = JSON.parse(
      readFileSync(packageJsonPath, "utf8"),
    ).version as string;
    const major = Number.parseInt(version.split(".", 1)[0] ?? "", 10);

    expect(
      major,
      "Vitest 3 can time out worker onTaskUpdate after 60s even when the test timeout is longer; keep Vitest >=4 for the canonical IR test",
    ).toBeGreaterThanOrEqual(4);
  });
});
