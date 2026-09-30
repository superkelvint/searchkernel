import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { SearchKernelError } from "../client/index.js";

const activeProcessGroups = new Set<number>();
let exitCleanupInstalled = false;
const HOST_STARTUP_TIMEOUT_MS = 60_000;
const MAX_HOST_OUTPUT_LENGTH = 8_192;

function signalProcessGroup(pid: number, signal: NodeJS.Signals): void {
  try {
    if (process.platform === "win32") process.kill(pid, signal);
    else process.kill(-pid, signal);
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    if (code !== "ESRCH") throw error;
  }
}

function installExitCleanup(): void {
  if (exitCleanupInstalled) return;
  exitCleanupInstalled = true;
  process.once("exit", () => {
    for (const pid of activeProcessGroups) {
      try {
        signalProcessGroup(pid, "SIGKILL");
      } catch {
        // Best-effort final cleanup only; explicit close() handles normal shutdown.
      }
    }
  });
}

function packageRoot(): string {
  return resolve(__dirname, "..", "..");
}

function repositoryRoot(): string {
  return resolve(packageRoot(), "..", "..");
}

function sourceRepositoryRoot(): string | null {
  const root = repositoryRoot();
  if (resolve(root, "clients", "typescript") !== packageRoot()) return null;
  if (!existsSync(resolve(root, "Cargo.toml"))) return null;
  if (!existsSync(resolve(root, "clients", "typescript", "package.json"))) return null;
  return root;
}

export function resolveHostBinary(): string {
  const configured = process.env.SEARCHKERNEL_LOCAL_HOST;
  if (configured) return configured;

  const packaged = resolve(packageRoot(), "bin", "searchkerneld");
  if (existsSync(packaged)) return packaged;

  const sourceRoot = sourceRepositoryRoot();
  if (sourceRoot !== null) {
    return resolve(sourceRoot, "target", "host", "debug", "searchkerneld");
  }

  throw new SearchKernelError(
    "managed local host binary is not bundled with @searchkernel/client; "
      + "set SEARCHKERNEL_LOCAL_HOST to an existing searchkerneld binary",
  );
}

export function environmentForHost(): NodeJS.ProcessEnv {
  const environment = { ...process.env };
  const sdkLib = environment.SEARCHKERNEL_SDK_LIB;
  if (sdkLib) {
    environment.LD_LIBRARY_PATH = environment.LD_LIBRARY_PATH
      ? `${sdkLib}:${environment.LD_LIBRARY_PATH}`
      : sdkLib;
  }
  return environment;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms));
}

function canConnect(socketPath: string): Promise<boolean> {
  return new Promise((resolveConnect) => {
    const socket = createConnection({ path: socketPath });
    let settled = false;
    const finish = (connected: boolean) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolveConnect(connected);
    };
    socket.setTimeout(100);
    socket.once("connect", () => finish(true));
    socket.once("error", () => finish(false));
    socket.once("timeout", () => finish(false));
  });
}

function waitForExit(child: ChildProcess, timeoutMs: number): Promise<boolean> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve(true);
  return new Promise((resolveWait) => {
    let settled = false;
    const finish = (exited: boolean) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      child.removeListener("exit", onExit);
      resolveWait(exited);
    };
    const onExit = () => finish(true);
    const timer = setTimeout(() => finish(false), timeoutMs);
    child.once("exit", onExit);
  });
}

export class ManagedLocalHost {
  private static readonly pool = new Map<
    string,
    { promise: Promise<ManagedLocalHost>; refs: number }
  >();

  readonly socketPath: string;
  readonly endpoint: string;
  private child: ChildProcess | null = null;
  private closed = false;
  private hostOutput = "";
  private poolKey: string | null = null;

  private constructor(
    readonly dataRoot: string,
    private readonly temporaryDirectory: string,
  ) {
    this.socketPath = join(temporaryDirectory, "searchkerneld.sock");
    this.endpoint = `unix://${this.socketPath}`;
  }

  static async start(dataRoot: string): Promise<ManagedLocalHost> {
    const key = resolve(dataRoot);
    let entry = this.pool.get(key);
    if (entry === undefined) {
      const promise = this.startUnpooled(key);
      entry = { promise, refs: 0 };
      this.pool.set(key, entry);
      promise.catch(() => {
        if (this.pool.get(key) === entry) this.pool.delete(key);
      });
    }

    entry.refs += 1;
    try {
      const host = await entry.promise;
      host.poolKey = key;
      return host;
    } catch (error) {
      entry.refs -= 1;
      throw error;
    }
  }

  private static async startUnpooled(dataRoot: string): Promise<ManagedLocalHost> {
    const temporaryDirectory = await mkdtemp(join(tmpdir(), "searchkernel-socket-"));
    const host = new ManagedLocalHost(dataRoot, temporaryDirectory);

    try {
      await mkdir(resolve(dataRoot), { recursive: true });
      host.spawn();
      await host.waitUntilReady();
      return host;
    } catch (error) {
      await host.close({ force: true });
      throw error;
    }
  }

  private spawn(): void {
    const binary = resolveHostBinary();
    if (!existsSync(binary)) {
      throw new SearchKernelError(
        `managed local host binary not found at ${binary}; set SEARCHKERNEL_LOCAL_HOST to an existing searchkerneld binary`,
      );
    }

    const args = [
      "--data-root",
      this.dataRoot,
      "--listen",
      this.endpoint,
    ];

    this.child = spawn(binary, args, {
      env: environmentForHost(),
      detached: true,
      stdio: ["ignore", "pipe", "pipe"],
    });

    this.child.stdout?.on("data", (chunk: Buffer | string) => {
      this.captureHostOutput("stdout", chunk);
    });
    this.child.stderr?.on("data", (chunk: Buffer | string) => {
      this.captureHostOutput("stderr", chunk);
    });

    if (!this.child.pid) {
      throw new SearchKernelError("managed local host did not start");
    }
    activeProcessGroups.add(this.child.pid);
    installExitCleanup();
    this.child.once("exit", () => {
      if (this.child?.pid) activeProcessGroups.delete(this.child.pid);
    });
  }

  private async waitUntilReady(): Promise<void> {
    const child = this.child;
    if (!child) throw new SearchKernelError("managed local host is not running");

    // The daemon opens or creates the Vespa index before binding its socket.
    // Cold native startup can therefore take longer than a short RPC timeout.
    const deadline = Date.now() + HOST_STARTUP_TIMEOUT_MS;
    while (Date.now() < deadline) {
      if (child.exitCode !== null || child.signalCode !== null) {
        throw this.startupError("managed local host exited during startup");
      }
      if (await canConnect(this.socketPath)) return;
      await delay(50);
    }
    throw this.startupError("managed local host did not open its gRPC socket before the startup deadline");
  }

  private captureHostOutput(stream: "stdout" | "stderr", chunk: Buffer | string): void {
    const text = chunk.toString();
    this.hostOutput = `${this.hostOutput}[${stream}] ${text}`.slice(-MAX_HOST_OUTPUT_LENGTH);
  }

  private startupError(message: string): SearchKernelError {
    const output = this.hostOutput.trim();
    return new SearchKernelError(
      output ? `${message}\nmanaged local host output:\n${output}` : message,
    );
  }

  async close(options: { force?: boolean } = {}): Promise<void> {
    if (this.closed) return;

    const poolKey = this.poolKey;
    if (poolKey !== null) {
      const entry = ManagedLocalHost.pool.get(poolKey);
      if (entry !== undefined) {
        if (!options.force && entry.refs > 1) {
          entry.refs -= 1;
          return;
        }
        ManagedLocalHost.pool.delete(poolKey);
      }
      this.poolKey = null;
    }

    this.closed = true;
    const child = this.child;
    this.child = null;
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      signalProcessGroup(child.pid, options.force ? "SIGKILL" : "SIGINT");
      if (!options.force && !(await waitForExit(child, 15_000))) {
        signalProcessGroup(child.pid, "SIGKILL");
        await waitForExit(child, 5_000);
      } else if (options.force) {
        await waitForExit(child, 5_000);
      }
      activeProcessGroups.delete(child.pid);
    }

    await rm(this.temporaryDirectory, { recursive: true, force: true });
  }
}
