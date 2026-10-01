import { lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync } from "node:fs";
import { EventEmitter } from "node:events";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { environmentForHost } from "../src/node/managed-local-host.js";

let sdkLib: string | undefined;
let loaderPath: string | undefined;
let localHost: string | undefined;
let legacyIndexPath: string | undefined;
let legacyDefinition: string | undefined;
let legacyDefinitionJson: string | undefined;

beforeEach(() => {
  sdkLib = process.env.SEARCHKERNEL_SDK_LIB;
  loaderPath = process.env.LD_LIBRARY_PATH;
  localHost = process.env.SEARCHKERNEL_LOCAL_HOST;
  legacyIndexPath = process.env.SEARCHKERNEL_INDEX_PATH;
  legacyDefinition = process.env.SEARCHKERNEL_INDEX_DEFINITION;
  legacyDefinitionJson = process.env.SEARCHKERNEL_INDEX_DEFINITION_JSON;
});

afterEach(() => {
  if (sdkLib === undefined) delete process.env.SEARCHKERNEL_SDK_LIB;
  else process.env.SEARCHKERNEL_SDK_LIB = sdkLib;
  if (loaderPath === undefined) delete process.env.LD_LIBRARY_PATH;
  else process.env.LD_LIBRARY_PATH = loaderPath;
  if (localHost === undefined) delete process.env.SEARCHKERNEL_LOCAL_HOST;
  else process.env.SEARCHKERNEL_LOCAL_HOST = localHost;
  if (legacyIndexPath === undefined) delete process.env.SEARCHKERNEL_INDEX_PATH;
  else process.env.SEARCHKERNEL_INDEX_PATH = legacyIndexPath;
  if (legacyDefinition === undefined) delete process.env.SEARCHKERNEL_INDEX_DEFINITION;
  else process.env.SEARCHKERNEL_INDEX_DEFINITION = legacyDefinition;
  if (legacyDefinitionJson === undefined) delete process.env.SEARCHKERNEL_INDEX_DEFINITION_JSON;
  else process.env.SEARCHKERNEL_INDEX_DEFINITION_JSON = legacyDefinitionJson;
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

  it("scrubs obsolete legacy single-index mode selectors from the child environment", () => {
    process.env.SEARCHKERNEL_INDEX_PATH = "/legacy/index";
    process.env.SEARCHKERNEL_INDEX_DEFINITION = "/legacy/definition.fb";
    process.env.SEARCHKERNEL_INDEX_DEFINITION_JSON = "/legacy/definition.json";

    const environment = environmentForHost();

    expect(environment.SEARCHKERNEL_INDEX_PATH).toBeUndefined();
    expect(environment.SEARCHKERNEL_INDEX_DEFINITION).toBeUndefined();
    expect(environment.SEARCHKERNEL_INDEX_DEFINITION_JSON).toBeUndefined();
    expect(process.env.SEARCHKERNEL_INDEX_PATH).toBe("/legacy/index");
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
      dataRoot: string,
      temporaryDirectory: string,
    ) => {
      captureHostOutput(stream: "stdout" | "stderr", chunk: string): void;
      startupError(message: string): Error;
    };
    const host = new Host("index", "/tmp/searchkernel-test");
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
      dataRoot: string,
      temporaryDirectory: string,
    ) => {
      child: { exitCode: number | null; signalCode: NodeJS.Signals | null } | null;
      socketPath: string;
      waitUntilReady(): Promise<void>;
    };
    const host = new Host("index", "/tmp/searchkernel-test");
    host.child = { exitCode: null, signalCode: null };
    host.socketPath = "/tmp/searchkernel-test/searchkerneld.sock";

    const readiness = host.waitUntilReady().then(
      () => undefined,
      (error: unknown) => error,
    );
    await vi.advanceTimersByTimeAsync(16_000);

    await expect(readiness).resolves.toBeUndefined();
  });

  it("reuses one daemon for the same data root until the last lease closes", async () => {
    const spawnMock = vi.fn(() => {
      const child = new EventEmitter() as EventEmitter & {
        pid: number;
        exitCode: number | null;
        signalCode: NodeJS.Signals | null;
        stdout: EventEmitter;
        stderr: EventEmitter;
      };
      child.pid = 999999;
      child.exitCode = null;
      child.signalCode = null;
      child.stdout = new EventEmitter();
      child.stderr = new EventEmitter();
      return child;
    });
    vi.doMock("node:child_process", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:child_process")>();
      return { ...actual, spawn: spawnMock };
    });
    vi.doMock("node:net", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:net")>();
      return {
        ...actual,
        createConnection: () => {
          const socket = new EventEmitter() as unknown as import("node:net").Socket;
          socket.setTimeout = vi.fn(() => socket);
          socket.destroy = vi.fn(() => socket);
          queueMicrotask(() => socket.emit("connect"));
          return socket;
        },
      };
    });
    vi.doMock("node:fs", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:fs")>();
      return { ...actual, existsSync: vi.fn(() => true) };
    });
    process.env.SEARCHKERNEL_LOCAL_HOST = "/tmp/searchkerneld";

    const { ManagedLocalHost } = await import("../src/node/managed-local-host.js");
    const first = await ManagedLocalHost.start("/tmp/searchkernel-shared-root");
    const second = await ManagedLocalHost.start("/tmp/searchkernel-shared-root");

    expect(first).toBe(second);
    expect(spawnMock).toHaveBeenCalledOnce();
    await first.close();
    expect((second as unknown as { closed: boolean }).closed).toBe(false);

    const child = (second as unknown as { child: { exitCode: number | null } }).child;
    child.exitCode = 0;
    await second.close();
    expect((second as unknown as { closed: boolean }).closed).toBe(true);
  });

  it("evicts a pooled daemon that exited unexpectedly before the next acquire", async () => {
    let nextPid = 900000;
    const spawnMock = vi.fn(() => {
      const child = new EventEmitter() as EventEmitter & {
        pid: number;
        exitCode: number | null;
        signalCode: NodeJS.Signals | null;
        stdout: EventEmitter;
        stderr: EventEmitter;
      };
      child.pid = nextPid++;
      child.exitCode = null;
      child.signalCode = null;
      child.stdout = new EventEmitter();
      child.stderr = new EventEmitter();
      return child;
    });
    vi.doMock("node:child_process", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:child_process")>();
      return { ...actual, spawn: spawnMock };
    });
    vi.doMock("node:net", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:net")>();
      return {
        ...actual,
        createConnection: () => {
          const socket = new EventEmitter() as unknown as import("node:net").Socket;
          socket.setTimeout = vi.fn(() => socket);
          socket.destroy = vi.fn(() => socket);
          queueMicrotask(() => socket.emit("connect"));
          return socket;
        },
      };
    });
    vi.doMock("node:fs", async (importOriginal) => {
      const actual = await importOriginal<typeof import("node:fs")>();
      return { ...actual, existsSync: vi.fn(() => true) };
    });
    process.env.SEARCHKERNEL_LOCAL_HOST = "/tmp/searchkerneld";

    const { ManagedLocalHost } = await import("../src/node/managed-local-host.js");
    const first = await ManagedLocalHost.start("/tmp/searchkernel-dead-root");
    const firstChild = (first as unknown as {
      child: { exitCode: number | null; signalCode: NodeJS.Signals | null };
    }).child;
    firstChild.exitCode = 23;

    const replacement = await ManagedLocalHost.start("/tmp/searchkernel-dead-root");
    expect(replacement).not.toBe(first);
    expect(spawnMock).toHaveBeenCalledTimes(2);

    const replacementChild = (replacement as unknown as {
      child: { exitCode: number | null; signalCode: NodeJS.Signals | null };
    }).child;
    replacementChild.exitCode = 0;
    await replacement.close();
  });

  it("does not commandeer a pre-existing empty unowned managed root", async () => {
    const root = mkdtempSync(join(tmpdir(), "searchkernel-ts-unowned-root-"));
    try {
      const dataRoot = join(root, ".searchkernel-managed");
      mkdirSync(dataRoot);
      const inode = lstatSync(dataRoot).ino;

      const { Index } = await import("../src/node/index.js");
      const { IndexDefinitionT } = await import(
        "../src/generated/flatbuffers/search-kernel/v2/index-definition.js"
      );
      await expect(
        Index.create(join(root, "index"), { fields: new IndexDefinitionT() }),
      ).rejects.toThrow(/not owned by Search Kernel/);

      expect(lstatSync(dataRoot).ino).toBe(inode);
      expect(readdirSync(dataRoot)).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("rejects a symlink legacy source before spawning the migration helper", async () => {
    const root = mkdtempSync(join(tmpdir(), "searchkernel-ts-migrate-symlink-"));
    try {
      const real = join(root, "real-index");
      const source = join(root, "legacy-index");
      const dataRoot = join(root, ".searchkernel-managed");
      mkdirSync(real);
      mkdirSync(dataRoot);
      symlinkSync(real, source, "dir");

      const { ManagedLocalHost } = await import("../src/node/managed-local-host.js");
      await expect(
        ManagedLocalHost.migrateLegacy(source, dataRoot, "local-test"),
      ).rejects.toThrow(/real directory/);
      expect(lstatSync(source).isSymbolicLink()).toBe(true);
      expect(lstatSync(real).isDirectory()).toBe(true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
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
      dataRoot: string,
      temporaryDirectory: string,
    ) => { spawn(): void };
    const host = new Host("relative-index", "/tmp/searchkernel-test");
    host.spawn();

    expect(spawnMock).toHaveBeenCalledOnce();
    const args = spawnMock.mock.calls[0]?.[1];
    expect(args).toContain("--data-root");
    expect(args).not.toContain("--index");
    expect(args).not.toContain("--definition");
    const options = spawnMock.mock.calls[0]?.[2];
    expect(options).not.toHaveProperty("cwd");
  });
});
