import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { SearchKernelError } from "../client/index.js";

const activeProcessGroups = new Set<number>();
let exitCleanupInstalled = false;

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

export function resolveHostBinary(): string {
  const configured = process.env.SEARCHKERNEL_LOCAL_HOST;
  if (configured) return configured;

  const packaged = resolve(packageRoot(), "bin", "searchkerneld");
  if (existsSync(packaged)) return packaged;

  return resolve(repositoryRoot(), "target", "debug", "searchkerneld");
}

function environmentForHost(): NodeJS.ProcessEnv {
  const environment = { ...process.env };
  let sdkLib = environment.SEARCHKERNEL_SDK_LIB;
  if (!sdkLib) {
    const candidate = resolve(
      repositoryRoot(),
      "core",
      "native",
      "sdk",
      "linux-x86_64",
      "lib",
    );
    if (existsSync(candidate)) sdkLib = candidate;
  }
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
  readonly socketPath: string;
  readonly endpoint: string;
  private child: ChildProcess | null = null;
  private closed = false;

  private constructor(
    readonly indexPath: string,
    private readonly temporaryDirectory: string,
    private readonly definitionPath: string | null,
  ) {
    this.socketPath = join(temporaryDirectory, "searchkerneld.sock");
    this.endpoint = `unix://${this.socketPath}`;
  }

  static async start(indexPath: string, definition: Uint8Array | null): Promise<ManagedLocalHost> {
    const temporaryDirectory = await mkdtemp(join(tmpdir(), "searchkernel-socket-"));
    const definitionPath = definition === null
      ? null
      : join(temporaryDirectory, "searchkernel-definition.fb");
    const host = new ManagedLocalHost(indexPath, temporaryDirectory, definitionPath);

    try {
      await mkdir(dirname(resolve(indexPath)), { recursive: true });
      if (definitionPath !== null) await writeFile(definitionPath, definition);
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
        `managed local host binary not found at ${binary}; build searchkerneld first`,
      );
    }

    const args = [
      "--index",
      this.indexPath,
      "--listen",
      this.endpoint,
    ];
    if (this.definitionPath !== null) {
      args.push("--definition", this.definitionPath);
    }

    this.child = spawn(binary, args, {
      cwd: repositoryRoot(),
      env: environmentForHost(),
      detached: true,
      stdio: "ignore",
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

    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null || child.signalCode !== null) {
        throw new SearchKernelError("managed local host exited during startup");
      }
      if (await canConnect(this.socketPath)) return;
      await delay(50);
    }
    throw new SearchKernelError("managed local host did not open its gRPC socket");
  }

  async close(options: { force?: boolean } = {}): Promise<void> {
    if (this.closed) return;
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
