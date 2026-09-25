import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let sdkLib: string | undefined;
let loaderPath: string | undefined;
let localHost: string | undefined;

beforeEach(() => {
  sdkLib = process.env.SEARCHKERNEL_SDK_LIB;
  loaderPath = process.env.LD_LIBRARY_PATH;
  localHost = process.env.SEARCHKERNEL_LOCAL_HOST;
});

afterEach(() => {
  if (sdkLib === undefined) delete process.env.SEARCHKERNEL_SDK_LIB;
  else process.env.SEARCHKERNEL_SDK_LIB = sdkLib;
  if (loaderPath === undefined) delete process.env.LD_LIBRARY_PATH;
  else process.env.LD_LIBRARY_PATH = loaderPath;
  if (localHost === undefined) delete process.env.SEARCHKERNEL_LOCAL_HOST;
  else process.env.SEARCHKERNEL_LOCAL_HOST = localHost;
  vi.resetModules();
  vi.unmock("node:fs");
  vi.unmock("node:child_process");
});

describe("managed-local native runtime contract", () => {
  it("does not probe the retired monolithic native SDK path", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/node/managed-local-host.ts"),
      "utf8",
    );

    expect(source).not.toContain('"native",\n      "sdk"');
    expect(source).not.toContain("core/native/sdk");
  });

  it("leaves the inherited loader path alone without an explicit adapter path", async () => {
    delete process.env.SEARCHKERNEL_SDK_LIB;
    process.env.LD_LIBRARY_PATH = "/existing";

    const { environmentForHost } = await import("../src/node/managed-local-host.js");
    expect(environmentForHost().LD_LIBRARY_PATH).toBe("/existing");
  });

  it("prepends the exact explicit adapter path without mutating process env", async () => {
    process.env.SEARCHKERNEL_SDK_LIB = "/exact/native-adapter/lib";
    process.env.LD_LIBRARY_PATH = "/existing";

    const { environmentForHost } = await import("../src/node/managed-local-host.js");
    const environment = environmentForHost();

    expect(environment.LD_LIBRARY_PATH).toBe(
      "/exact/native-adapter/lib:/existing",
    );
    expect(process.env.LD_LIBRARY_PATH).toBe("/existing");
  });
});

describe("managed-local host binary resolution", () => {
  it("fails closed outside a Search Kernel source checkout when no host is configured or bundled", async () => {
    vi.doMock("node:fs", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:fs")>();
      return {
        ...actual,
        existsSync: vi.fn(() => false),
      };
    });

    const { resolveHostBinary } = await import("../src/node/managed-local-host.js");

    expect(() => resolveHostBinary()).toThrow(/SEARCHKERNEL_LOCAL_HOST/);
  });

  it("uses the repository host target only for a positively identified source checkout", async () => {
    vi.doMock("node:fs", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:fs")>();
      return {
        ...actual,
        existsSync: vi.fn((path: import("node:fs").PathLike) => {
          const value = String(path);
          return value.endsWith("/Cargo.toml")
            || value.endsWith("/clients/typescript/package.json");
        }),
      };
    });

    const { resolveHostBinary } = await import("../src/node/managed-local-host.js");
    expect(resolveHostBinary()).toMatch(/\/target\/host\/debug\/searchkerneld$/);
  });

  it("does not make managed-local execution depend on a repository working directory", async () => {
    const spawnMock = vi.fn((
      _binary: string,
      _args: readonly string[],
      _options: Record<string, unknown>,
    ) => ({
      pid: 999999,
      exitCode: null,
      signalCode: null,
      once: vi.fn(),
    }));

    vi.doMock("node:child_process", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:child_process")>();
      return {
        ...actual,
        spawn: spawnMock,
      };
    });
    vi.doMock("node:fs", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:fs")>();
      return {
        ...actual,
        existsSync: vi.fn(() => true),
      };
    });
    process.env.SEARCHKERNEL_LOCAL_HOST = "/tmp/searchkerneld";

    const { ManagedLocalHost } = await import("../src/node/managed-local-host.js");
    const Host = ManagedLocalHost as unknown as new (
      indexPath: string,
      temporaryDirectory: string,
      definitionPath: string | null,
    ) => { spawn(): void };
    const host = new Host("relative-index", "/tmp/searchkernel-test", null);
    host.spawn();

    expect(spawnMock).toHaveBeenCalledOnce();
    const options = spawnMock.mock.calls[0]?.[2];
    expect(options).not.toHaveProperty("cwd");
  });
});
