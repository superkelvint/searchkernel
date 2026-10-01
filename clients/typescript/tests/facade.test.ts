import { beforeAll, describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Index, SearchKernelError } from "../src/client/index.js";
import { int64, uint64 } from "../src/client/values.js";
import {
  agg,
  desc,
  facetRequest,
  g,
  Group,
  rootAggregationRequest,
} from "../src/ergonomic/grouping.js";
import { q } from "../src/ergonomic/query.js";
import { paritySafe } from "../examples/parity-json.js";
import { Keyword, RankProfile, TensorField, Text } from "../src/ergonomic/schema.js";
import { DocumentT } from "../src/semantic.js";
import { ErrorCode } from "../src/semantic.js";
import { ErrorT } from "../src/semantic.js";
import { GroupingOperationKind } from "../src/semantic.js";
import { GroupingExpressionKind } from "../src/semantic.js";
import { GroupingSourceKind } from "../src/semantic.js";
import { InsertResultT } from "../src/semantic.js";
import { SearchRequestT } from "../src/semantic.js";
import { RequestEnvelope } from "../src/_wire/v1/generated/flatbuffers/search-kernel/protocol/request-envelope.js";
import { SearchRequest } from "../src/_wire/v1/generated/flatbuffers/search-kernel/wire/search-request.js";
import { QuerySourceKind } from "../src/semantic.js";
import { QueryExprKind } from "../src/semantic.js";
import { ValueKind } from "../src/semantic.js";
import type { GrpcWebTransport } from "../src/transport/grpc-web.js";
import { ByteBuffer } from "flatbuffers";
import { packObject, packRequest } from "../src/_wire/v1/codec.js";

let semanticSnapshotBinary: string | undefined;

function prepareSemanticSnapshotBinary(): void {
  const repository = resolve(process.cwd(), "../..");
  const metadata = execFileSync(
    "cargo",
    ["metadata", "--no-deps", "--format-version", "1"],
    { cwd: repository, encoding: "utf8" },
  );
  const binary = join(
    JSON.parse(metadata).target_directory as string,
    "debug",
    "searchkernel-semantic-snapshot",
  );
  if (!existsSync(binary)) {
    execFileSync(
      "cargo",
      [
        "build",
        "--quiet",
        "--no-default-features",
        "-p",
        "searchkernel-testkit",
        "--bin",
        "searchkernel-semantic-snapshot",
      ],
      { cwd: repository, stdio: "ignore" },
    );
  }
  semanticSnapshotBinary = binary;
}

function canonicalSnapshot(request: SearchRequestT): unknown {
  if (semanticSnapshotBinary === undefined) {
    throw new Error("semantic snapshot oracle was not prepared");
  }
  const repository = resolve(process.cwd(), "../..");
  const fixture = join(repository, "reference/api-conformance/fixtures/indexes/product-v2.json");
  const directory = mkdtempSync(join(tmpdir(), "searchkernel-ir-"));
  const requestPath = join(directory, "request.bfbs");
  try {
    writeFileSync(requestPath, packObject(request));
    const output = execFileSync(
      semanticSnapshotBinary,
      [
        "--index-json",
        fixture,
        "--request",
        requestPath,
      ],
      { cwd: repository, encoding: "utf8" },
    );
    return JSON.parse(output) as unknown;
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function index(): Index {
  return Index.connect("unused", "test", { execute: async () => new Uint8Array() } as unknown as GrpcWebTransport);
}

describe("ergonomic facade", () => {
  beforeAll(() => {
    prepareSemanticSnapshotBinary();
  }, 300_000);

  // SKC: filter.textual/basic kinds=ergonomic runtime=shared
  // SKC: search.sorting/basic kinds=ergonomic runtime=shared
  // SKC: search.pagination/basic kinds=ergonomic runtime=shared
  // SKC: result.docsum_field_projection/basic kinds=ergonomic runtime=shared
  it("builds search/filter/sort/pagination/ranking/query-input requests", () => {
    const request = index()
      .search("running shoes")
      .filter("price <= :max", { max: 120 })
      .sort("price", "asc")
      .ranking("product-search")
      .input("user_id", int64("9223372036854775807"))
      .rankProperty("foo", "bar")
      .fields("title", "price")
      .page(2, 20)
      .build();

    expect(request.query?.sourceType).toBe(QuerySourceKind.SearchKernelTextQuery);
    expect(request.filters?.textual).toHaveLength(1);
    expect(request.filters?.textual[0]?.bindings[0]?.value?.kindType).toBe(ValueKind.Int64Value);
    expect(request.sort).toHaveLength(1);
    expect(request.ranking?.profile).toBe("product-search");
    expect(request.inputs).toHaveLength(1);
    expect(request.presentation?.fields?.fields).toEqual(["title", "price"]);
    expect(request.offset?.value).toBe(20);
    expect(request.limit?.value).toBe(20);
  });

  it("requires an explicit field sort direction", () => {
    const builder = index().search("running shoes");
    expect(() => (builder.sort as (field: string) => unknown)("price")).toThrow(TypeError);
    if (false) {
      // @ts-expect-error Field sort direction is required by the public contract.
      builder.sort("price");
    }

    const ascending = index().search("running shoes").sort("price", "asc").build();
    const descending = index().search("running shoes").sort("price", "desc").build();
    expect(ascending.sort?.[0]?.direction).toBe(1);
    expect(descending.sort?.[0]?.direction).toBe(2);
    expect(canonicalSnapshot(ascending)).toMatchObject({ sort: [{ direction: "asc" }] });
    expect(canonicalSnapshot(descending)).toMatchObject({ sort: [{ direction: "desc" }] });
  });

  it("rejects non-string rank properties and preserves string values", () => {
    const builder = index().search("running shoes");
    expect(() => builder.rankProperty("enabled", false as unknown as string)).toThrow(TypeError);
    expect(builder.rankProperty("source", "catalog").build().ranking?.properties?.[0]?.value)
      .toBe("catalog");
  });

  it("preserves absent and explicit ranking profiles with rank properties", () => {
    const implicit = index().search("running shoes").rankProperty("source", "catalog").build();
    expect(implicit.ranking?.profile).toBeNull();
    expect(implicit.ranking?.properties?.[0]?.value).toBe("catalog");
    const wire = packRequest("search", implicit);
    const envelope = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(wire));
    const search = envelope.payload(new SearchRequest());
    expect(search?.ranking()?.profile()).toBeNull();

    const explicit = index().search("running shoes").ranking("default")
      .rankProperty("source", "catalog").build();
    expect(explicit.ranking?.profile).toBe("default");
  });

  it("omits deprecated filter vectors when packing textual filters", () => {
    const request = index().search("running shoes").filter("price <= :max", { max: 120 }).build();
    const bytes = packRequest("search", request);
    const envelope = RequestEnvelope.getRootAsRequestEnvelope(new ByteBuffer(bytes));
    const search = envelope.payload(new SearchRequest());
    const filters = search?.filters();
    expect(filters).not.toBeNull();
    expect(filters?.bb?.__offset(filters.bb_pos, 6)).toBe(0);
    expect(filters?.bb?.__offset(filters.bb_pos, 8)).toBe(0);
    expect(filters?.bb?.__offset(filters.bb_pos, 10)).not.toBe(0);
    expect(filters?.textualLength()).toBe(1);
    expect(filters?.textual(0)?.source()).toBe("price <= :max");
  });

  // SKC: query.nearest_neighbor/basic kinds=ergonomic runtime=shared
  // SKC: grouping.structured/basic kinds=ergonomic runtime=shared
  // SKC: grouping.aggregations/basic kinds=ergonomic runtime=shared
  it("ports typed query, group, facet and root aggregation construction", () => {
    const request = index()
      .query(q.allOf(q.contains("title", "rust"), q.nearestNeighbor("embedding", "q_embedding")))
      .group(
        Group.by("category")
          .limit(20)
          .orderBy(desc(agg.count()))
          .output(agg.count().as("products"), agg.avg("price").as("avg_price"))
          .subgroup(Group.by("brand").limit(10).output(agg.count().as("products"))),
      )
      .facet("brand", { name: "brands", limit: 5, sort: "count_desc" })
      .aggregate(agg.max("price").as("max_price"))
      .input("big", uint64("18446744073709551615"))
      .build();

    expect(request.query?.sourceType).toBe(QuerySourceKind.TypedQuery);
    expect(request.grouping).toHaveLength(3);
    expect(request.grouping.every((group) => group.sourceType === GroupingSourceKind.StructuredGroupingSource)).toBe(true);
    const root = request.grouping[0]?.source as { root?: { operations?: Array<{ operationType?: GroupingOperationKind }> } };
    expect(root.root?.operations?.[0]?.operationType).toBe(GroupingOperationKind.GroupingGroupOperation);
  });

  // SKC: query.prefix_suffix_substring/basic kinds=ergonomic runtime=shared
  it("lowers literal term/prefix/substring/suffix helpers to canonical query kinds", () => {
    const term = (q.term("title", "quick").source as any).query;
    expect(term.exprType).toBe(QueryExprKind.QueryContains);
    expect(term.expr.item.exprType).toBe(QueryExprKind.TermQuery);

    for (const [query, pattern] of [
      [q.prefix("brand", "A.c+"), "A\\.c\\+.*"],
      [q.substring("brand", "A.c+"), ".*A\\.c\\+.*"],
      [q.suffix("brand", "A.c+"), ".*A\\.c\\+"],
    ] as const) {
      const expression = (query.source as any).query;
      expect(expression.exprType).toBe(QueryExprKind.QueryMatches);
      expect(expression.expr.pattern).toBe(pattern);
    }
  });

  it("preserves unsafe 64-bit parity integers as decimal strings", () => {
    expect(paritySafe(BigInt(Number.MAX_SAFE_INTEGER))).toBe(Number.MAX_SAFE_INTEGER);
    expect(paritySafe(BigInt(Number.MAX_SAFE_INTEGER) + 1n)).toBe("9007199254740992");
    expect(paritySafe(9007199254740993n)).toBe("9007199254740993");
    expect(paritySafe({ count: 9007199254740993n })).toEqual({ count: "9007199254740993" });
  });

  it("supports grouping expressions and catches local pagination misuse", () => {
    const expression = g.field("price").mul(agg.count()).as("revenue");
    expect(expression.label).toBe("revenue");
    expect(() => index().browse().page(2, 10).offset(20)).toThrow(/cannot follow page/);
  });

  it("compares pagination and grouping helpers through canonical Rust IR", () => {
    const page = index().browse().page(2, 20).build();
    const explicit = index().browse().limit(20).offset(20).build();
    expect(canonicalSnapshot(page)).toEqual(canonicalSnapshot(explicit));
    const firstPage = index().browse().page(1, 20).build();
    const firstPageExplicit = index().browse().limit(20).offset(0).build();
    expect(canonicalSnapshot(firstPage)).toEqual(canonicalSnapshot(firstPageExplicit));
    const zeroPage = index().browse().page(1, 0).build();
    const zeroPageExplicit = index().browse().limit(0).offset(0).build();
    expect(canonicalSnapshot(zeroPage)).toEqual(canonicalSnapshot(zeroPageExplicit));

    const helper = index()
      .browse()
      .group(Group.by("category").limit(3).output(agg.count().as("products")))
      .facet("brand", { name: "brands", limit: 5, sort: "count_desc" })
      .aggregate(agg.count().as("products"))
      .build();
    const explicitGrouping = new SearchRequestT(
      helper.query,
      helper.searchFields,
      helper.filters,
      helper.sort,
      [
        Group.by("category").limit(3).output(agg.count().as("products")).toRequest(),
        facetRequest("brand", 5, "count_desc", "brands"),
        rootAggregationRequest([agg.count().as("products")]),
      ],
      helper.ranking,
      helper.inputs,
      helper.limit,
      helper.offset,
      helper.timeoutMs,
      helper.queryModel,
      helper.presentation,
      helper.properties,
      helper.queryProfile,
      helper.searchChain,
    );
    expect(canonicalSnapshot(helper)).toEqual(canonicalSnapshot(explicitGrouping));
  }, 120_000);

  it("maps portable error codes to stable names", () => {
    const error = new SearchKernelError(
      "unsupported",
      new ErrorT(ErrorCode.Unsupported, "unsupported", [], false),
    );
    expect(error.code).toBe("unsupported");
  });
  it("ports the public field and rank-profile helpers", () => {
    const text = new Text({ stored: true, searchable: true }).definition("title");
    const keyword = new Keyword({ filterable: true }).definition("brand");
    const tensor = new TensorField("tensor<float>(x[4])", { searchable: true }).definition("embedding");
    const rank = new RankProfile("semantic")
      .firstPhase("closeness(field, embedding)")
      .secondPhase("attribute(inventory)")
      .rerankCount(3)
      .matchFeature("bm25(title)")
      .queryInput("query_embedding", "tensor<float>(x[4])")
      .definition();

    expect(text.name).toBe("title");
    expect(keyword.name).toBe("brand");
    expect(tensor.name).toBe("embedding");
    expect(rank.name).toBe("semantic");
    expect(rank.inputs).toHaveLength(1);
    expect(rank.secondPhase?.expression).toBe("attribute(inventory)");
    expect(rank.secondPhase?.rerankCount?.value).toBe(3);
  });

  it("validates second-phase expressions and rerank counts", () => {
    expect(() => new RankProfile("empty").secondPhase("  ")).toThrow(/must not be empty/);
    expect(() => new RankProfile("negative").rerankCount(-1)).toThrow(/uint32/);
    expect(() => new RankProfile("too-large").rerankCount(0x1_0000_0000)).toThrow(/uint32/);
  });


  it("enforces uint32 bounds before FlatBuffers packing", () => {
    expect(() => index().browse().limit(0xffff_ffff)).not.toThrow();
    expect(() => index().browse().limit(0x1_0000_0000)).toThrow(/4294967295/);
    expect(() => index().browse().offset(0x1_0000_0000)).toThrow(/4294967295/);
    expect(() => index().browse().page(2, 0xffff_ffff)).toThrow(/uint32/);
    expect(() => Group.by("brand").limit(0x1_0000_0000)).toThrow(/4294967295/);
    expect(() => Group.by("brand").precision(0x1_0000_0000)).toThrow(/4294967295/);
    expect(() => index().browse().facet("brand", { limit: 0x1_0000_0000 })).toThrow(/4294967295/);
  });

  it("preserves explicit facet limit zero and canonical facet name", () => {
    const request = index().browse().facet("brand", { name: "brands", limit: 0 }).build();
    const source = request.grouping[0]?.source as {
      root?: {
        label?: string | null;
        operations?: Array<{ operation?: { operations?: Array<{ operation?: { value?: { value?: number } } }> } }>;
      };
    };
    const each = source.root?.operations?.[1]?.operation;
    expect(each?.operations?.[0]?.operation?.value?.value).toBe(0);
    expect(source.root?.label).toBe("brands");
  });

  it("normalizes all facet sort modes and multiple facet names canonically", () => {
    const sorts = ["count_desc", "count_asc", "value_asc", "value_desc"] as const;
    const expectedKinds = [
      GroupingExpressionKind.GroupingUnaryExpression,
      GroupingExpressionKind.GroupingAggregationExpression,
      GroupingExpressionKind.GroupingAttributeExpression,
      GroupingExpressionKind.GroupingUnaryExpression,
    ];

    const request = sorts.reduce(
      (builder, sort, index) =>
        builder.facet(index % 2 === 0 ? "brand" : "category", {
          name: `facet-${index}`,
          limit: index,
          sort,
        }),
      index().browse(),
    ).build();

    expect(request.grouping).toHaveLength(4);
    request.grouping.forEach((group, index) => {
      const source = group.source as {
        root?: {
          label?: string | null;
          operations?: Array<{
            operation?: {
              operations?: Array<{
                operation?: {
                  value?: { value?: number };
                  expressions?: Array<{ expression?: { exprType?: GroupingExpressionKind } }>;
                };
              }>;
            };
          }>;
        };
      };
      const each = source.root?.operations?.[1]?.operation;
      expect(source.root?.label).toBe(`facet-${index}`);
      expect(each?.operations?.[0]?.operation?.value?.value).toBe(index);
      expect(each?.operations?.[1]?.operation?.expressions?.[0]?.expression?.exprType)
        .toBe(expectedKinds[index]);
    });
  });

  it("exposes operation-specific low-level response types", () => {
    const client = index().client;
    const typedInsert: (doc: DocumentT) => Promise<InsertResultT> = client.insert.bind(client);
    expect(typeof typedInsert).toBe("function");
    const request: SearchRequestT = index().browse().build();
    expect(request).toBeInstanceOf(SearchRequestT);
  });

});


it("uses canonical Unicode scalar ordering for filter binding keys", () => {
  const bmp = "\uE000";
  const nonBmp = "\u{10000}";
  const request = index().browse().filter("x = :a", { [nonBmp]: 1, [bmp]: 2 }).build();
  expect(request.filters?.textual[0]?.bindings.map((entry) => entry.key)).toEqual([bmp, nonBmp]);
});
