import { Index as RemoteIndex, Client, SearchKernelError } from "../client/index.js";
import type { ClientTransport } from "../client/index.js";
import { existsSync } from "node:fs";
import { lstat, mkdir, readFile, rmdir, unlink, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { IndexDefinitionT } from "../semantic.js";
import { FieldDefinitionT } from "../semantic.js";
import { RankProfileDefinitionT } from "../semantic.js";
import { NodeGrpcTransport } from "../transport/grpc-node.js";
import { ManagedLocalHost } from "./managed-local-host.js";

export interface FieldDefinitionBuilder {
  definition(name: string): FieldDefinitionT;
}

export interface RankProfileDefinitionBuilder {
  definition(): RankProfileDefinitionT;
}

export interface CreateIndexOptions {
  fields: Record<string, FieldDefinitionBuilder | FieldDefinitionT> | IndexDefinitionT;
  rankProfiles?: readonly (RankProfileDefinitionBuilder | RankProfileDefinitionT)[];
}

function indexDefinition(options: CreateIndexOptions): IndexDefinitionT {
  if (options.fields instanceof IndexDefinitionT) {
    if (options.rankProfiles !== undefined) {
      throw new TypeError("rankProfiles cannot accompany an IndexDefinitionT");
    }
    return options.fields;
  }

  const definition = new IndexDefinitionT();
  definition.name = "index";
  definition.fields = Object.entries(options.fields).map(([name, specification]) =>
    specification instanceof FieldDefinitionT ? specification : specification.definition(name),
  );
  definition.rankProfiles = (options.rankProfiles ?? []).map((profile) =>
    profile instanceof RankProfileDefinitionT ? profile : profile.definition(),
  );
  return definition;
}

const MANAGED_DATA_ROOT_NAME = ".searchkernel-managed";
const MANAGED_MARKER_NAME = ".searchkernel-managed-target";
const MANAGED_ROOT_MARKER_NAME = ".searchkernel-data-root";
const MANAGED_ROOT_MARKER_VALUE = "searchkernel-managed-v1";

function localLocation(path: string): { indexPath: string; dataRoot: string; target: string } {
  const indexPath = resolve(path);
  const target = `local-${createHash("sha256").update(indexPath).digest("hex").slice(0, 32)}`;
  return {
    indexPath,
    dataRoot: join(dirname(indexPath), MANAGED_DATA_ROOT_NAME),
    target,
  };
}

function managedRootError(dataRoot: string): SearchKernelError {
  return new SearchKernelError(
    `managed-local data root is not owned by Search Kernel: ${dataRoot}`,
  );
}

async function validateManagedDataRoot(dataRoot: string): Promise<void> {
  const marker = join(dataRoot, MANAGED_ROOT_MARKER_NAME);
  let rootStat;
  let markerStat;
  try {
    rootStat = await lstat(dataRoot);
    markerStat = await lstat(marker);
  } catch {
    throw managedRootError(dataRoot);
  }
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) throw managedRootError(dataRoot);
  if (markerStat.isSymbolicLink() || !markerStat.isFile()) throw managedRootError(dataRoot);
  let value: string;
  try {
    value = await readFile(marker, "utf8");
  } catch {
    throw managedRootError(dataRoot);
  }
  if (
    value !== MANAGED_ROOT_MARKER_VALUE
    && value !== `${MANAGED_ROOT_MARKER_VALUE}\n`
    && value !== `${MANAGED_ROOT_MARKER_VALUE}\r\n`
  ) {
    throw managedRootError(dataRoot);
  }
}

async function prepareManagedDataRoot(dataRoot: string): Promise<void> {
  await mkdir(dirname(dataRoot), { recursive: true });
  const marker = join(dataRoot, MANAGED_ROOT_MARKER_NAME);
  let created = false;
  try {
    await mkdir(dataRoot);
    created = true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
  }

  if (created) {
    try {
      await writeFile(marker, `${MANAGED_ROOT_MARKER_VALUE}\n`, { flag: "wx" });
      await validateManagedDataRoot(dataRoot);
      return;
    } catch (error) {
      await unlink(marker).catch(() => undefined);
      await rmdir(dataRoot).catch(() => undefined);
      throw error;
    }
  }

  const deadline = Date.now() + 2_000;
  for (;;) {
    let rootStat;
    try {
      rootStat = await lstat(dataRoot);
    } catch {
      throw managedRootError(dataRoot);
    }
    if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
      throw managedRootError(dataRoot);
    }
    try {
      await lstat(marker);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw managedRootError(dataRoot);
      }
      if (Date.now() >= deadline) throw managedRootError(dataRoot);
      await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
      continue;
    }
    await validateManagedDataRoot(dataRoot);
    return;
  }
}

async function reserveManagedPath(indexPath: string, target: string): Promise<void> {
  await mkdir(dirname(indexPath), { recursive: true });
  let created = false;
  try {
    await mkdir(indexPath);
    created = true;
    await writeFile(join(indexPath, MANAGED_MARKER_NAME), `${target}\n`, { flag: "wx" });
  } catch (error) {
    if (created) {
      await unlink(join(indexPath, MANAGED_MARKER_NAME)).catch(() => undefined);
      await rmdir(indexPath).catch(() => undefined);
    }
    throw error;
  }
}

async function ensureManagedMarker(indexPath: string, target: string): Promise<void> {
  try {
    await mkdir(indexPath);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    const pathStat = await lstat(indexPath);
    if (pathStat.isSymbolicLink() || !pathStat.isDirectory()) {
      throw new SearchKernelError(
        `managed-local identity path is not a real directory: ${indexPath}`,
      );
    }
  }

  const marker = join(indexPath, MANAGED_MARKER_NAME);
  try {
    await writeFile(marker, `${target}\n`, { flag: "wx" });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
  }
  const markerStat = await lstat(marker);
  if (markerStat.isSymbolicLink() || !markerStat.isFile()) {
    throw new SearchKernelError(`unsafe managed-local marker: ${marker}`);
  }
  if ((await readFile(marker, "utf8")).trim() !== target) {
    throw new SearchKernelError(`managed-local marker target mismatch at ${marker}`);
  }
}

async function cleanupManagedReservation(indexPath: string, target: string): Promise<void> {
  const marker = join(indexPath, MANAGED_MARKER_NAME);
  try {
    if ((await readFile(marker, "utf8")).trim() !== target) return;
    await unlink(marker);
    await rmdir(indexPath);
  } catch {
    // Best-effort rollback only. Never remove caller-created contents.
  }
}

/** Node-specific Index that can own a local searchkerneld process. */
export class Index extends RemoteIndex {
  private owner: ManagedLocalHost | null;
  private closed = false;

  private constructor(client: Client, owner: ManagedLocalHost | null = null) {
    super(client);
    this.owner = owner;
  }

  static connect(endpoint: string, target: string, transport?: ClientTransport): Index;
  static connect(endpoint: string, transport?: ClientTransport): Index;
  static connect(
    endpoint: string,
    targetOrTransport?: string | ClientTransport,
    selectedTransport?: ClientTransport,
  ): Index {
    const target = typeof targetOrTransport === "string" ? targetOrTransport : undefined;
    const transport = typeof targetOrTransport === "string" ? selectedTransport : targetOrTransport;
    return new Index(new Client(transport ?? new NodeGrpcTransport(endpoint), target));
  }

  static create(endpoint: string, target: string, definition: IndexDefinitionT | CreateIndexOptions): Promise<Index>;
  static create(path: string, options: CreateIndexOptions): Promise<Index>;
  static async create(
    location: string,
    targetOrOptions: string | CreateIndexOptions,
    remoteDefinition?: IndexDefinitionT | CreateIndexOptions,
  ): Promise<Index> {
    if (typeof targetOrOptions === "string") {
      const client = new Client(
        new NodeGrpcTransport(location),
        targetOrOptions,
      );
      const definition = remoteDefinition instanceof IndexDefinitionT
        ? remoteDefinition
        : indexDefinition(remoteDefinition as CreateIndexOptions);
      try {
        const result = await client.createIndex(definition);
        if (result.index === null) throw new Error("CreateIndexResult.index is missing");
        return new Index(client);
      } catch (error) {
        await client.close();
        throw error;
      }
    }

    const { indexPath, dataRoot, target } = localLocation(location);
    await prepareManagedDataRoot(dataRoot);
    await reserveManagedPath(indexPath, target);
    let owner: ManagedLocalHost | null = null;
    let client: Client | null = null;
    try {
      owner = await ManagedLocalHost.start(dataRoot);
      client = new Client(new NodeGrpcTransport(owner.endpoint), target);
      const result = await client.createIndex(indexDefinition(targetOrOptions));
      if (result.index === null) throw new Error("CreateIndexResult.index is missing");
      return new Index(client, owner);
    } catch (error) {
      await client?.close();
      await owner?.close();
      await cleanupManagedReservation(indexPath, target);
      throw error;
    }
  }

  static async open(location: string, remoteTarget?: string): Promise<Index> {
    if (remoteTarget !== undefined) {
      const client = new Client(new NodeGrpcTransport(location), remoteTarget);
      try {
        const result = await client.openIndex();
        if (result.index === null) throw new Error("OpenIndexResult.index is missing");
        return new Index(client);
      } catch (error) {
        await client.close();
        throw error;
      }
    }

    const { indexPath, dataRoot, target } = localLocation(location);
    await prepareManagedDataRoot(dataRoot);
    await ManagedLocalHost.migrateLegacy(indexPath, dataRoot, target);
    if (existsSync(join(dataRoot, target)) && !existsSync(indexPath)) {
      await ensureManagedMarker(indexPath, target);
    }
    const owner = await ManagedLocalHost.start(dataRoot);
    const client = new Client(new NodeGrpcTransport(owner.endpoint), target);
    try {
      const result = await client.openIndex();
      if (result.index === null) throw new Error("OpenIndexResult.index is missing");
      return new Index(client, owner);
    } catch (error) {
      await client.close();
      await owner.close();
      throw error;
    }
  }

  static delete(endpoint: string, target: string, transport?: ClientTransport): Promise<void>;
  static delete(path: string): Promise<void>;
  static async delete(
    location: string,
    remoteTarget?: string,
    transport?: ClientTransport,
  ): Promise<void> {
    if (remoteTarget !== undefined) {
      return RemoteIndex.delete(
        location,
        remoteTarget,
        transport ?? new NodeGrpcTransport(location),
      );
    }

    const { indexPath, dataRoot, target } = localLocation(location);
    await prepareManagedDataRoot(dataRoot);
    await ManagedLocalHost.migrateLegacy(indexPath, dataRoot, target);
    if (existsSync(join(dataRoot, target)) && !existsSync(indexPath)) {
      await ensureManagedMarker(indexPath, target);
    }

    const owner = await ManagedLocalHost.start(dataRoot);
    let deleted = false;
    try {
      await RemoteIndex.delete(
        owner.endpoint,
        target,
        new NodeGrpcTransport(owner.endpoint),
      );
      deleted = true;
    } finally {
      try {
        await owner.close();
      } finally {
        if (deleted) await cleanupManagedReservation(indexPath, target);
      }
    }
  }

  override async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    await super.close();
    if (this.owner !== null) {
      await this.owner.close();
      this.owner = null;
    }
  }
}
