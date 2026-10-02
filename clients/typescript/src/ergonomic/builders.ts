import { ArithmeticUpdateT } from "../semantic.js";
import { ArithmeticUpdateOperator } from "../semantic.js";
import { AssignUpdateT } from "../semantic.js";
import { ClearUpdateT } from "../semantic.js";
import { FieldPathT } from "../semantic.js";
import { FieldProjectionT } from "../semantic.js";
import { FilterClauseT } from "../semantic.js";
import { FilterRequestT } from "../semantic.js";
import { LiteralQueryInputT } from "../semantic.js";
import { MatchAllQueryT } from "../semantic.js";
import { OptionalBoolT } from "../semantic.js";
import { OptionalUInt32T } from "../semantic.js";
import { PatchOperationKind } from "../semantic.js";
import { PatchOperationT } from "../semantic.js";
import { PatchRequestT } from "../semantic.js";
import { PresentationRequestT } from "../semantic.js";
import { QueryInputKind } from "../semantic.js";
import { QueryInputT } from "../semantic.js";
import { QuerySourceKind } from "../semantic.js";
import { QuerySourceT } from "../semantic.js";
import { RankingRequestT } from "../semantic.js";
import { SearchKernelTextQueryT } from "../semantic.js";
import { SearchRequestT } from "../semantic.js";
import { SetFieldT } from "../semantic.js";
import { StringEntryT } from "../semantic.js";
import { SortDirection } from "../semantic.js";
import { SortExprT } from "../semantic.js";
import { SortTargetKind } from "../semantic.js";
import { SortTargetT } from "../semantic.js";
import { FieldSortTargetT } from "../semantic.js";
import { UnsetFieldT } from "../semantic.js";
import { UpdateOperationKind } from "../semantic.js";
import { UpdateOperationT } from "../semantic.js";
import { UpdateRequestT } from "../semantic.js";
import { ValueEntryT } from "../semantic.js";
import { YqlQueryT } from "../semantic.js";
import { SearchResults } from "../client/results.js";
import { value, type ValueInput } from "../client/values.js";
import type { Index } from "../client/index.js";
import type { ExecuteOptions } from "../transport/types.js";
import { UINT32_MAX, assertUInt32, comparePortableKeys } from "../internal/scalars.js";
import {
  Group,
  GroupingExpr,
  GroupingOutput,
  facetRequest,
  rootAggregationRequest,
  type FacetSort,
} from "./grouping.js";

interface FacetSpec {
  field: string;
  name: string;
  limit?: number;
  sort?: FacetSort;
}

// The generated object packer emits every vector, including empty deprecated
// vectors. The IR distinguishes an absent legacy filter field from a present
// one, so textual-only requests need a packer that omits those fields on the
// wire.
class TextualFilterRequest extends FilterRequestT {
  constructor(textual: FilterClauseT[]) {
    super([], [], [], textual);
    (this as any)._textualOnly = true;
  }
}

export interface FacetOptions {
  name?: string;
  limit?: number;
  sort?: FacetSort;
}

export class SearchBuilder {
  protected readonly filters: Array<[string, Record<string, ValueInput>]> = [];
  protected selectedFields?: string[];
  protected limitValue?: number;
  protected offsetValue?: number;
  protected paged = false;
  protected readonly sorts: SortExprT[] = [];
  protected readonly groups: Group[] = [];
  protected readonly aggregates: GroupingOutput[] = [];
  protected readonly facets: FacetSpec[] = [];
  protected rankingProfile?: string;
  protected summaryName?: string;
  protected readonly inputs = new Map<string, ValueInput>();
  protected readonly rankProperties = new Map<string, string>();
  protected readonly properties = new Map<string, ValueInput>();

  constructor(protected readonly index: Index, protected readonly queryValue: unknown) {}

  filter(source: string, bindings: Record<string, ValueInput> = {}): this {
    this.filters.push([source, { ...bindings }]);
    return this;
  }

  fields(...fields: string[]): this {
    this.selectedFields = fields;
    return this;
  }

  sort(field: string, direction: "asc" | "desc"): this {
    if (direction !== "asc" && direction !== "desc") {
      throw new TypeError("sort direction must be 'asc' or 'desc'");
    }
    this.sorts.push(
      new SortExprT(
        new SortTargetT(SortTargetKind.FieldSortTarget, new FieldSortTargetT(field)),
        direction === "asc" ? SortDirection.Asc : SortDirection.Desc,
      ),
    );
    return this;
  }

  facet(field: string, options: FacetOptions = {}): this {
    const limit = options.limit;
    if (limit !== undefined) assertUInt32(limit, "facet limit");
    this.facets.push({ field, name: options.name || field, limit, sort: options.sort });
    return this;
  }

  group(group: Group): this {
    this.groups.push(group);
    return this;
  }

  aggregate(expression: GroupingExpr | GroupingOutput): this {
    this.aggregates.push(expression instanceof GroupingOutput ? expression : expression.as(""));
    return this;
  }

  ranking(profile: string): this {
    this.rankingProfile = profile;
    return this;
  }

  summary(name: string): this {
    this.summaryName = name;
    return this;
  }

  input(name: string, input: ValueInput): this {
    this.inputs.set(name, input);
    return this;
  }

  rankProperty(name: string, item: string): this {
    if (typeof item !== "string") {
      throw new TypeError("rank property value must be a string");
    }
    this.rankProperties.set(name, item);
    return this;
  }

  property(name: string, item: ValueInput): this {
    this.properties.set(name, item);
    return this;
  }

  limit(value_: number): this {
    if (this.paged) throw new RangeError("limit cannot follow page()");
    this.limitValue = assertUInt32(value_, "limit");
    return this;
  }

  offset(value_: number): this {
    if (this.paged) throw new RangeError("offset cannot follow page()");
    this.offsetValue = assertUInt32(value_, "offset");
    return this;
  }

  page(number: number, size: number): this {
    if (
      !Number.isSafeInteger(number)
      || number < 1
      || this.limitValue !== undefined
      || this.offsetValue !== undefined
    ) {
      throw new RangeError("invalid page() combination");
    }
    const checkedSize = assertUInt32(size, "page size");
    const pageEnd = BigInt(number) * BigInt(checkedSize);
    if (pageEnd > BigInt(UINT32_MAX)) throw new RangeError("page range exceeds uint32 range");
    const offset = BigInt(number - 1) * BigInt(checkedSize);
    this.paged = true;
    this.offsetValue = Number(offset);
    this.limitValue = checkedSize;
    return this;
  }

  build(): SearchRequestT {
    const query = this.queryValue === null
      ? new QuerySourceT(QuerySourceKind.MatchAllQuery, new MatchAllQueryT())
      : typeof this.queryValue === "string"
        ? new QuerySourceT(QuerySourceKind.SearchKernelTextQuery, new SearchKernelTextQueryT(this.queryValue))
        : this.queryValue as QuerySourceT;

    const filters = this.filters.length
      ? new TextualFilterRequest(
          this.filters.map(
            ([source, bindings]) => new FilterClauseT(
              source,
              Object.keys(bindings)
                .sort(comparePortableKeys)
                .map((name) => new ValueEntryT(name, value(bindings[name]!))),
            ),
          ),
        )
      : null;

    const inputs = [...this.inputs]
      .sort(([a], [b]) => comparePortableKeys(a, b))
      .map(([name, item]) => new QueryInputT(
        name,
        QueryInputKind.LiteralQueryInput,
        new LiteralQueryInputT(value(item)),
      ));

    const ranking = this.rankingProfile !== undefined || this.rankProperties.size
      ? new RankingRequestT(
          this.rankingProfile ?? null,
          [...this.rankProperties]
            .sort(([a], [b]) => comparePortableKeys(a, b))
            .map(([key, item]) => new StringEntryT(key, item)),
        )
      : null;

    const grouping = this.groups.map((group) => group.toRequest());
    for (const facet of this.facets) {
      grouping.push(facetRequest(facet.field, facet.limit ?? 10, facet.sort ?? "count_desc", facet.name));
    }
    if (this.aggregates.length) grouping.push(rootAggregationRequest(this.aggregates));

    return new SearchRequestT(
      query,
      null,
      filters,
      this.sorts,
      grouping,
      ranking,
      inputs,
      this.limitValue === undefined ? null : new OptionalUInt32T(this.limitValue),
      this.offsetValue === undefined ? null : new OptionalUInt32T(this.offsetValue),
      null,
      null,
      this.selectedFields || this.summaryName !== undefined
        ? new PresentationRequestT(this.summaryName ?? null,
            this.selectedFields ? new FieldProjectionT(this.selectedFields) : null)
        : null,
      [...this.properties]
        .sort(([a], [b]) => comparePortableKeys(a, b))
        .map(([key, item]) => new ValueEntryT(key, value(item))),
    );
  }

  async execute(options?: ExecuteOptions): Promise<SearchResults> {
    const result = new SearchResults(await this.index.client.search(this.build(), options));
    for (const facet of this.facets) result.projectFacet(facet.name, facet.name, facet.field);
    return result;
  }
}

export class YqlBuilder extends SearchBuilder {
  constructor(index: Index, source: string) {
    super(index, new QuerySourceT(QuerySourceKind.YqlQuery, new YqlQueryT(source)));
  }
}

export class PatchBuilder {
  private readonly operations: PatchOperationT[] = [];

  constructor(private readonly index: Index, private readonly id: string) {}

  set(field: string, item: ValueInput): this {
    this.operations.push(
      new PatchOperationT(PatchOperationKind.SetField, new SetFieldT(field, value(item))),
    );
    return this;
  }

  unset(field: string): this {
    this.operations.push(
      new PatchOperationT(PatchOperationKind.UnsetField, new UnsetFieldT(field)),
    );
    return this;
  }

  execute(options?: ExecuteOptions) {
    return this.index.client.patch(new PatchRequestT(this.id, this.operations), options);
  }
}

export class UpdateBuilder {
  private readonly operations: UpdateOperationT[] = [];
  private conditionValue?: string;
  private createValue?: boolean;

  constructor(private readonly index: Index, private readonly id: string) {}

  set(field: string, item: ValueInput): this {
    this.operations.push(
      new UpdateOperationT(
        new FieldPathT(field),
        UpdateOperationKind.AssignUpdate,
        new AssignUpdateT(value(item)),
      ),
    );
    return this;
  }

  assign(field: string, item: ValueInput): this { return this.set(field, item); }

  unset(field: string): this {
    this.operations.push(
      new UpdateOperationT(
        new FieldPathT(field),
        UpdateOperationKind.ClearUpdate,
        new ClearUpdateT(),
      ),
    );
    return this;
  }

  clear(field: string): this { return this.unset(field); }

  private arithmetic(field: string, item: ValueInput, operator: ArithmeticUpdateOperator): this {
    this.operations.push(
      new UpdateOperationT(
        new FieldPathT(field),
        UpdateOperationKind.ArithmeticUpdate,
        new ArithmeticUpdateT(operator, value(item)),
      ),
    );
    return this;
  }

  increment(field: string, item: ValueInput): this { return this.arithmetic(field, item, ArithmeticUpdateOperator.Increment); }
  decrement(field: string, item: ValueInput): this { return this.arithmetic(field, item, ArithmeticUpdateOperator.Decrement); }
  multiply(field: string, item: ValueInput): this { return this.arithmetic(field, item, ArithmeticUpdateOperator.Multiply); }
  divide(field: string, item: ValueInput): this { return this.arithmetic(field, item, ArithmeticUpdateOperator.Divide); }

  condition(expression: string): this {
    this.conditionValue = expression;
    return this;
  }

  create(enabled = true): this {
    this.createValue = enabled;
    return this;
  }

  execute(options?: ExecuteOptions) {
    return this.index.client.update(
      new UpdateRequestT(
        this.id,
        this.operations,
        this.conditionValue ?? null,
        this.createValue === undefined ? null : new OptionalBoolT(this.createValue),
      ),
      options,
    );
  }
}
