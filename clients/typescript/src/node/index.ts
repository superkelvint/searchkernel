import { Index as RemoteIndex, Client } from "../client/index.js";
import { realpathSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";
import { IndexDefinitionT } from "../generated/flatbuffers/search-kernel/v2/index-definition.js";
import { FieldDefinitionT } from "../generated/flatbuffers/search-kernel/v2/field-definition.js";
import { RankProfileDefinitionT } from "../generated/flatbuffers/search-kernel/v2/rank-profile-definition.js";
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

function localLocation(path: string): { dataRoot: string; target: string } {
  let canonicalPath: string;
  try {
    canonicalPath = realpathSync(path);
  } catch {
    canonicalPath = resolve(path);
  }
  const target = basename(canonicalPath);
  if (!/^[a-z0-9][a-z0-9_-]{0,127}$/.test(target)) {
    throw new TypeError(
      "managed-local index directory name must match [a-z0-9][a-z0-9_-]{0,127}",
    );
  }
  return { dataRoot: dirname(canonicalPath), target };
}

/** Node-specific Index that can own a local searchkerneld process. */
export class Index extends RemoteIndex {
  private owner: ManagedLocalHost | null;
  private closed = false;

  private constructor(client: Client, owner: ManagedLocalHost | null = null) {
    super(client);
    this.owner = owner;
  }

  static connect(endpoint: string, target: string): Index {
    return new Index(new Client(new NodeGrpcTransport(endpoint), target));
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

    const { dataRoot, target } = localLocation(location);
    const owner = await ManagedLocalHost.start(dataRoot);
    const client = new Client(new NodeGrpcTransport(owner.endpoint), target);
    try {
      const result = await client.createIndex(indexDefinition(targetOrOptions));
      if (result.index === null) throw new Error("CreateIndexResult.index is missing");
      return new Index(client, owner);
    } catch (error) {
      await client.close();
      await owner.close();
      throw error;
    }
  }

  static async open(location: string, target?: string): Promise<Index> {
    if (target !== undefined) {
      const client = new Client(new NodeGrpcTransport(location), target);
      try {
        const result = await client.openIndex();
        if (result.index === null) throw new Error("OpenIndexResult.index is missing");
        return new Index(client);
      } catch (error) {
        await client.close();
        throw error;
      }
    }

    const { dataRoot, target } = localLocation(location);
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
