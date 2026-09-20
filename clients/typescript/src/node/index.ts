import { Index as RemoteIndex, Client } from "../client/index.js";
import { IndexDefinitionT } from "../generated/flatbuffers/search-kernel/v2/index-definition.js";
import { FieldDefinitionT } from "../generated/flatbuffers/search-kernel/v2/field-definition.js";
import { RankProfileDefinitionT } from "../generated/flatbuffers/search-kernel/v2/rank-profile-definition.js";
import { packObject } from "../protocol/envelope.js";
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

/** Node-specific Index that can own a local searchkerneld process. */
export class Index extends RemoteIndex {
  private owner: ManagedLocalHost | null;
  private closed = false;

  private constructor(client: Client, owner: ManagedLocalHost | null = null) {
    super(client);
    this.owner = owner;
  }

  static connect(endpoint: string): Index {
    return new Index(new Client(new NodeGrpcTransport(endpoint)));
  }

  static async create(path: string, options: CreateIndexOptions): Promise<Index> {
    const owner = await ManagedLocalHost.start(path, packObject(indexDefinition(options)));
    return new Index(new Client(new NodeGrpcTransport(owner.endpoint)), owner);
  }

  static async open(path: string): Promise<Index> {
    const owner = await ManagedLocalHost.start(path, null);
    return new Index(new Client(new NodeGrpcTransport(owner.endpoint)), owner);
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
