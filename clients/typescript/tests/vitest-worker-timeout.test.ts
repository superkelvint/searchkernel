import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Vitest worker RPC regression", () => {
  it("uses a Vitest major with the slow onTaskUpdate worker fix", () => {
    const packageJsonPath = resolve(process.cwd(), "node_modules", "vitest", "package.json");
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
