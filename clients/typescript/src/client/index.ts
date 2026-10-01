import { AnalyzeFieldTargetT } from "../semantic.js";
import { AnalyzeRequestT } from "../semantic.js";
import { AnalyzeTarget } from "../semantic.js";
import { AnalyzeResultT } from "../semantic.js";
import { DeleteRequestT } from "../semantic.js";
import { DeleteResultT } from "../semantic.js";
import { DescribeRequestT } from "../semantic.js";
import { DescribeResultT } from "../semantic.js";
import { DocumentT } from "../semantic.js";
import { ErrorCode } from "../semantic.js";
import { ErrorT } from "../semantic.js";
import { FieldProjectionT } from "../semantic.js";
import { GetRequestT } from "../semantic.js";
import { GetResultT } from "../semantic.js";
import { InsertRequestT } from "../semantic.js";
import { InsertResultT } from "../semantic.js";
import { InspectQueryRequestT } from "../semantic.js";
import { InspectQueryResultT } from "../semantic.js";
import { PatchRequestT } from "../semantic.js";
import { PatchResultT } from "../semantic.js";
import { SearchRequestT } from "../semantic.js";
import { SearchResultT } from "../semantic.js";
import { UpdateRequestT } from "../semantic.js";
import { UpdateResultT } from "../semantic.js";
import { UpsertRequestT } from "../semantic.js";
import { UpsertResultT } from "../semantic.js";
import { CreateIndexRequestT } from "../semantic.js";
import { OpenIndexRequestT } from "../semantic.js";
import { CreateIndexResultT } from "../semantic.js";
import { OpenIndexResultT } from "../semantic.js";
import { IndexDefinitionT } from "../semantic.js";
import { ValueEntryT } from "../semantic.js";
import { packRequest, unpackResponse, type Operation, type OperationRequestMap, ProtocolError } from "../protocol/envelope.js";
import { GrpcWebTransport } from "../transport/grpc-web.js";
import { TransportError, type ExecuteOptions } from "../transport/types.js";
import { decodeValueEntries, value, type ValueInput } from "./values.js";
import { comparePortableKeys } from "../internal/scalars.js";
import { PatchBuilder, SearchBuilder, UpdateBuilder, YqlBuilder } from "../ergonomic/builders.js";
import type { TypedQuery } from "../ergonomic/query.js";

const ERROR_CODE_NAMES: Record<number, string> = {
  [ErrorCode.Unspecified]: "unspecified",
  [ErrorCode.InvalidArgument]: "invalid_argument",
  [ErrorCode.NotFound]: "not_found",
  [ErrorCode.AlreadyExists]: "already_exists",
  [ErrorCode.FailedPrecondition]: "failed_precondition",
  [ErrorCode.Conflict]: "conflict",
  [ErrorCode.Unsupported]: "unsupported",
  [ErrorCode.ResourceExhausted]: "resource_exhausted",
  [ErrorCode.Internal]: "internal",
  [ErrorCode.Unavailable]: "unavailable",
  [ErrorCode.DeadlineExceeded]: "deadline_exceeded",
};

export class SearchKernelError extends Error {
  override name = "SearchKernelError";

  constructor(message: string, readonly error?: ErrorT, readonly cause?: unknown) {
    super(message);
  }

  get code(): string {
    return ERROR_CODE_NAMES[this.error?.code ?? ErrorCode.Internal] ?? "internal";
  }

  get details(): readonly unknown[] {
    return this.error?.details ?? [];
  }

  get retryable(): boolean {
    return this.error?.retryable ?? false;
  }
}

export type DocumentInput = { _id: string; [field: string]: ValueInput };

function document(id: string, fields: Record<string, ValueInput>): DocumentT {
  if (!id) throw new TypeError("document id must be a non-empty string");
  if (!fields || typeof fields !== "object") throw new TypeError("document fields must be an object");
  return new DocumentT(
    id,
    Object.keys(fields)
      .sort(comparePortableKeys)
      .map((key) => new ValueEntryT(key, value(fields[key]!))),
  );
}

export interface OperationResponseMap {
  insert: InsertResultT;
  upsert: UpsertResultT;
  patch: PatchResultT;
  update: UpdateResultT;
  delete: DeleteResultT;
  get: GetResultT;
  search: SearchResultT;
  analyze: AnalyzeResultT;
  describe: DescribeResultT;
  inspectQuery: InspectQueryResultT;
  createIndex: CreateIndexResultT;
  openIndex: OpenIndexResultT;
}

export interface ClientTransport {
  execute(payload: Uint8Array, options?: ExecuteOptions): Promise<Uint8Array>;
  close?(): void | Promise<void>;
}

export class Client {
  constructor(readonly transport: ClientTransport, readonly target?: string) {}

  async execute<K extends Operation>(
    operation: K,
    request: OperationRequestMap[K],
    options?: ExecuteOptions,
  ): Promise<OperationResponseMap[K]> {
    try {
      const decoded = unpackResponse(
        await this.transport.execute(packRequest(operation, request, this.target), options),
        operation,
      );
      if (decoded.isError) {
        const error = decoded.payload as ErrorT;
        throw new SearchKernelError(
          typeof error.message === "string" ? error.message : "Search Kernel request failed",
          error,
        );
      }
      return decoded.payload as OperationResponseMap[K];
    } catch (error) {
      if (error instanceof SearchKernelError) throw error;
      if (error instanceof TransportError || error instanceof ProtocolError) throw error;
      throw new SearchKernelError(
        error instanceof Error ? error.message : "Search Kernel request failed",
        undefined,
        error,
      );
    }
  }

  insert(doc: DocumentT, options?: ExecuteOptions) {
    return this.execute("insert", new InsertRequestT(doc), options);
  }

  upsert(doc: DocumentT, condition?: string, options?: ExecuteOptions) {
    return this.execute("upsert", new UpsertRequestT(doc, condition ?? null), options);
  }

  delete(id: string, condition?: string, options?: ExecuteOptions) {
    return this.execute("delete", new DeleteRequestT(id, condition ?? null), options);
  }

  get(id: string, projection?: readonly string[], options?: ExecuteOptions) {
    return this.execute("get", new GetRequestT(
      id,
      projection !== undefined ? new FieldProjectionT([...projection]) : null,
    ), options);
  }

  patch(request: PatchRequestT, options?: ExecuteOptions) {
    return this.execute("patch", request, options);
  }

  update(request: UpdateRequestT, options?: ExecuteOptions) {
    return this.execute("update", request, options);
  }

  search(request: SearchRequestT, options?: ExecuteOptions): Promise<SearchResultT> {
    return this.execute("search", request, options);
  }

  analyze(text: string, language?: string, field?: string, options?: ExecuteOptions) {
    return this.execute(
      "analyze",
      new AnalyzeRequestT(
        field !== undefined ? AnalyzeTarget.AnalyzeFieldTarget : AnalyzeTarget.NONE,
        field !== undefined ? new AnalyzeFieldTargetT(field) : null,
        text,
        language ?? null,
      ),
      options,
    );
  }

  describe(options?: ExecuteOptions) {
    return this.execute("describe", new DescribeRequestT(), options);
  }

  inspectQuery(request: SearchRequestT, options?: ExecuteOptions): Promise<InspectQueryResultT> {
    return this.execute("inspectQuery", new InspectQueryRequestT(request), options);
  }

  createIndex(definition: IndexDefinitionT, options?: ExecuteOptions): Promise<CreateIndexResultT> {
    return this.execute("createIndex", new CreateIndexRequestT(definition), options);
  }

  openIndex(options?: ExecuteOptions): Promise<OpenIndexResultT> {
    return this.execute("openIndex", new OpenIndexRequestT(), options);
  }

  async close(): Promise<void> {
    await this.transport.close?.();
  }
}

/** Public remote-index façade. Browser SDK intentionally has no local daemon lifecycle. */
export class Index {
  readonly client: Client;

  protected constructor(client: Client) {
    this.client = client;
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
    return new Index(new Client(transport ?? new GrpcWebTransport(endpoint), target));
  }

  static async create(
    endpoint: string,
    target: string,
    definition: IndexDefinitionT,
    transport?: ClientTransport,
  ): Promise<Index> {
    const client = new Client(transport ?? new GrpcWebTransport(endpoint), target);
    try {
      const result = await client.createIndex(definition);
      if (result.index === null) throw new ProtocolError("CreateIndexResult.index is missing");
      return new Index(client);
    } catch (error) {
      await client.close();
      throw error;
    }
  }

  static async open(
    endpoint: string,
    target: string,
    transport?: ClientTransport,
  ): Promise<Index> {
    const client = new Client(transport ?? new GrpcWebTransport(endpoint), target);
    try {
      const result = await client.openIndex();
      if (result.index === null) throw new ProtocolError("OpenIndexResult.index is missing");
      return new Index(client);
    } catch (error) {
      await client.close();
      throw error;
    }
  }

  insert(id: string, fields: Record<string, ValueInput>, options?: ExecuteOptions): Promise<InsertResultT>;
  insert(documentInput: DocumentInput, options?: ExecuteOptions): Promise<InsertResultT>;
  insert(
    idOrDocument: string | DocumentInput,
    fieldsOrOptions?: Record<string, ValueInput> | ExecuteOptions,
    options?: ExecuteOptions,
  ): Promise<InsertResultT> {
    if (typeof idOrDocument === "string") {
      if (!fieldsOrOptions) {
        throw new TypeError("insert(id, fields) requires a fields object");
      }
      return this.client.insert(
        document(idOrDocument, fieldsOrOptions as Record<string, ValueInput>),
        options,
      );
    }

    const { _id, ...fields } = idOrDocument;
    return this.client.insert(document(_id, fields), fieldsOrOptions as ExecuteOptions | undefined);
  }

  upsert(
    id: string,
    fields: Record<string, ValueInput>,
    condition?: string,
    options?: ExecuteOptions,
  ): Promise<UpsertResultT>;
  upsert(documentInput: DocumentInput, condition?: string, options?: ExecuteOptions): Promise<UpsertResultT>;
  upsert(
    idOrDocument: string | DocumentInput,
    fieldsOrCondition?: Record<string, ValueInput> | string,
    conditionOrOptions?: string | ExecuteOptions,
    options?: ExecuteOptions,
  ): Promise<UpsertResultT> {
    if (typeof idOrDocument === "string") {
      if (!fieldsOrCondition || typeof fieldsOrCondition === "string") {
        throw new TypeError("upsert(id, fields) requires a fields object");
      }
      return this.client.upsert(
        document(idOrDocument, fieldsOrCondition),
        typeof conditionOrOptions === "string" ? conditionOrOptions : undefined,
        typeof conditionOrOptions === "string" ? options : conditionOrOptions,
      );
    }

    const { _id, ...fields } = idOrDocument;
    return this.client.upsert(
      document(_id, fields),
      typeof fieldsOrCondition === "string" ? fieldsOrCondition : undefined,
      conditionOrOptions as ExecuteOptions | undefined,
    );
  }

  async delete(id: string, condition?: string, options?: ExecuteOptions): Promise<boolean> {
    const result = await this.client.delete(id, condition, options);
    return result.deleted;
  }

  async get(
    id: string,
    projection?: readonly string[],
    options?: ExecuteOptions,
  ): Promise<Record<string, unknown> | null> {
    const result = await this.client.get(id, projection, options);
    return result.document
      ? {
          _id: result.document.id,
          ...decodeValueEntries(result.document.fields, "document field"),
        }
      : null;
  }

  search(query: string): SearchBuilder {
    return new SearchBuilder(this, query);
  }

  browse(): SearchBuilder {
    return new SearchBuilder(this, null);
  }

  query(query: TypedQuery): SearchBuilder {
    return new SearchBuilder(this, query);
  }

  yql(source: string): YqlBuilder {
    return new YqlBuilder(this, source);
  }

  patch(id: string): PatchBuilder {
    return new PatchBuilder(this, id);
  }

  update(id: string): UpdateBuilder {
    return new UpdateBuilder(this, id);
  }

  analyze(text: string, language?: string, field?: string, options?: ExecuteOptions) {
    return this.client.analyze(text, language, field, options);
  }

  describe(options?: ExecuteOptions) {
    return this.client.describe(options);
  }

  inspectQuery(request: SearchRequestT, options?: ExecuteOptions): Promise<InspectQueryResultT> {
    return this.client.inspectQuery(request, options);
  }

  async close(): Promise<void> {
    await this.client.close();
  }
}
