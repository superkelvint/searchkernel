import { readFileSync } from "node:fs";
import { EventEmitter } from "node:events";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { environmentForHost } from "../src/node/managed-local-host.js";

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
  vi.useRealTimers();
  vi.unmock("node:fs");
  vi.unmock("node:net");
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

  it("leaves the inherited loader path alone without an explicit adapter path", () => {
    delete process.env.SEARCHKERNEL_SDK_LIB;
    process.env.LD_LIBRARY_PATH = "/existing";

    expect(environmentForHost().LD_LIBRARY_PATH).toBe("/existing");
  });

  it("prepends the exact explicit adapter path without mutating process env", () => {
    process.env.SEARCHKERNEL_SDK_LIB = "/exact/native-adapter/lib";
    process.env.LD_LIBRARY_PATH = "/existing";

    const environment = environmentForHost();

    expect(environment.LD_LIBRARY_PATH).toBe(
      "/exact/native-adapter/lib:/existing",
    );
    expect(process.env.LD_LIBRARY_PATH).toBe("/existing");
  });
});

describe("managed-local host binary resolution", () => {
  it("includes captured daemon output in startup errors", async () => {
    const { ManagedLocalHost } = await import("../src/node/managed-local-host.js");
    const Host = ManagedLocalHost as unknown as new (
      indexPath: string,
      temporaryDirectory: string,
      definitionPath: string | null,
    ) => {
      captureHostOutput(stream: "stdout" | "stderr", chunk: string): void;
      startupError(message: string): Error;
    };
    const host = new Host("index", "/tmp/searchkernel-test", null);
    host.captureHostOutput("stderr", "native index open failed");

    expect(host.startupError("managed local host exited during startup").message).toContain(
      "native index open failed",
    );
  });

  it("continues waiting when native index startup takes longer than 15 seconds", async () => {
    vi.useFakeTimers();
    let connectionAttempts = 0;
    vi.doMock("node:net", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:net")>();
      return {
        ...actual,
        createConnection: () => {
          const attempt = ++connectionAttempts;
          const socket = new EventEmitter() as unknown as import("node:net").Socket;
          socket.setTimeout = vi.fn(() => socket);
          socket.destroy = vi.fn(() => socket);
          queueMicrotask(() => {
            socket.emit(attempt > 300 ? "connect" : "error");
          });
          return socket;
        },
      };
    });

    const { ManagedLocalHost } = await import("../src/node/managed-local-host.js");
    const Host = ManagedLocalHost as unknown as new (
      indexPath: string,
      temporaryDirectory: string,
      definitionPath: string | null,
    ) => {
      child: { exitCode: number | null; signalCode: NodeJS.Signals | null } | null;
      socketPath: string;
      waitUntilReady(): Promise<void>;
    };
    const host = new Host("index", "/tmp/searchkernel-test", null);
    host.child = { exitCode: null, signalCode: null };
    host.socketPath = "/tmp/searchkernel-test/searchkerneld.sock";

    const readiness = host.waitUntilReady().then(
      () => undefined,
      (error: unknown) => error,
    );
    await vi.advanceTimersByTimeAsync(16_000);

    await expect(readiness).resolves.toBeUndefined();
  });

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
