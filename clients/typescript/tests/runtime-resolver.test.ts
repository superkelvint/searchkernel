import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

const packageRoot = resolve(process.cwd());
const descriptorPath = join(packageRoot, "runtime.json");
const hadDescriptor = existsSync(descriptorPath);
const originalDescriptor = hadDescriptor ? readFileSync(descriptorPath, "utf8").toString() : undefined;
const runtimeDirEnv = process.env.SEARCHKERNEL_RUNTIME_DIR;
let tempRoot: string | undefined;

afterEach(() => {
  if (originalDescriptor === undefined) {
    if (existsSync(descriptorPath)) unlinkSync(descriptorPath);
  } else {
    writeFileSync(descriptorPath, originalDescriptor);
  }
  if (runtimeDirEnv === undefined) delete process.env.SEARCHKERNEL_RUNTIME_DIR;
  else process.env.SEARCHKERNEL_RUNTIME_DIR = runtimeDirEnv;
  if (tempRoot) rmSync(tempRoot, { recursive: true, force: true });
  tempRoot = undefined;
  vi.restoreAllMocks();
});

function createRuntime(mutator?: (root: string) => void): { root: string; descriptor: Record<string, unknown> } {
  tempRoot = mkdtempSync(join(tmpdir(), "sk-node-runtime-"));
  const root = join(tempRoot, "runtime");
  mkdirSync(join(root, "bin"), { recursive: true });
  mkdirSync(join(root, "lib"), { recursive: true });
  writeFileSync(join(root, "bin/searchkerneld"), "daemon\n", { mode: 0o755 });
  writeFileSync(join(root, "bin/searchkerneld.real"), "real daemon\n", { mode: 0o755 });
  writeFileSync(join(root, "lib/libsearchkernel_vespa.so.2"), "adapter\n");

  const conformance = JSON.parse(readFileSync(resolve(packageRoot, "../../build/consumer-runtime/conformance.json"), "utf8"));
  const vector = conformance.vectors[0];
  const identity = vector.identity_material as Record<string, unknown>;
  const payloadHash = createHash("sha256");
  for (const relative of ["bin/searchkerneld", "bin/searchkerneld.real", "lib/libsearchkernel_vespa.so.2"]) {
    const name = Buffer.from(relative);
    const data = readFileSync(join(root, relative));
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(name.length));
    payloadHash.update(length).update(name);
    length.writeBigUInt64BE(BigInt(data.length));
    payloadHash.update(length).update(data);
  }
  const descriptor = {
    format_version: conformance.format_version,
    ...identity,
    runtime_identity: vector.runtime_identity,
    artifact_url: `https://example.invalid/searchkernel-runtime-${vector.runtime_identity}-linux-x86_64-glibc.tar.gz`,
    artifact_sha256: "a".repeat(64),
  };
  writeFileSync(join(root, "manifest.json"), JSON.stringify({
    format_version: 1,
    ...identity,
    runtime_identity: vector.runtime_identity,
    payload_sha256: payloadHash.digest("hex"),
  }));
  mutator?.(root);
  writeFileSync(descriptorPath, JSON.stringify(descriptor));
  process.env.SEARCHKERNEL_RUNTIME_DIR = root;
  return { root, descriptor };
}

describe("Node consumer-runtime conformance", () => {
  it("consumes every identity and cache-path vector", () => {
    const conformance = JSON.parse(readFileSync(resolve(packageRoot, "../../build/consumer-runtime/conformance.json"), "utf8"));
    for (const vector of conformance.vectors) {
      const canonical = JSON.stringify(Object.fromEntries(Object.entries(vector.identity_material).sort(([a], [b]) => a.localeCompare(b))));
      const identity = `skrt-v1-${createHash("sha256").update(canonical).digest("hex")}`;
      expect(identity).toBe(vector.runtime_identity);
      expect(`objects/${identity}`).toBe(vector.cache_relative_path);
      expect(`locks/${identity}.lock`).toBe(vector.lock_relative_path);
    }
  });

  it("rejects an x86-64 musl host instead of accepting the glibc runtime", async () => {
    createRuntime();
    vi.spyOn(process.report, "getReport").mockReturnValue(
      { header: {} } as ReturnType<typeof process.report.getReport>,
    );
    const { resolveRuntime } = await import("../src/node/runtime-resolver.js");
    await expect(resolveRuntime()).rejects.toThrow("platform-mismatch");
  });

  const conformance = JSON.parse(readFileSync(resolve(packageRoot, "../../build/consumer-runtime/conformance.json"), "utf8"));
  for (const fixture of conformance.runtime_validation_cases) {
    it(`rejects ${fixture.name} using the shared validation fixture`, async () => {
      createRuntime((root) => {
        const manifestPath = join(root, "manifest.json");
        const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
        switch (fixture.mutation) {
          case "manifest-format": manifest.format_version = 99; break;
          case "non-executable": chmodSync(join(root, "bin/searchkerneld.real"), 0o644); break;
          case "escaping-symlink": {
            const outside = join(tempRoot!, "outside.so");
            writeFileSync(outside, "outside\n");
            rmSync(join(root, "lib/libsearchkernel_vespa.so.2"));
            symlinkSync(outside, join(root, "lib/libsearchkernel_vespa.so.2"));
            break;
          }
          case "identity-mismatch": manifest.searchkernel_revision = "different"; break;
          case "payload-digest-mismatch": manifest.payload_sha256 = "0".repeat(64); break;
          default: throw new Error(`unknown runtime fixture mutation: ${fixture.mutation}`);
        }
        if (fixture.mutation !== "non-executable" && fixture.mutation !== "escaping-symlink") {
          writeFileSync(manifestPath, JSON.stringify(manifest));
        }
      });
      const { resolveRuntime } = await import("../src/node/runtime-resolver.js");
      await expect(resolveRuntime()).rejects.toThrow(fixture.expected_error);
    });
  }
});
