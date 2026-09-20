import { describe, expect, it } from "vitest";
import { Index, SearchKernelError } from "../src/client/index.js";
import { int64, uint64 } from "../src/client/values.js";
import { agg, desc, g, Group } from "../src/ergonomic/grouping.js";
import { q } from "../src/ergonomic/query.js";
import { Keyword, RankProfile, TensorField, Text } from "../src/ergonomic/schema.js";
import { DocumentT } from "../src/generated/flatbuffers/search-kernel/v2/document.js";
import { ErrorCode } from "../src/generated/flatbuffers/search-kernel/v2/error-code.js";
import { ErrorT } from "../src/generated/flatbuffers/search-kernel/v2/error.js";
import { GroupingOperationKind } from "../src/generated/flatbuffers/search-kernel/v2/grouping-operation-kind.js";
import { GroupingSourceKind } from "../src/generated/flatbuffers/search-kernel/v2/grouping-source-kind.js";
import { InsertResultT } from "../src/generated/flatbuffers/search-kernel/v2/insert-result.js";
import { SearchRequestT } from "../src/generated/flatbuffers/search-kernel/v2/search-request.js";
import { QuerySourceKind } from "../src/generated/flatbuffers/search-kernel/v2/query-source-kind.js";
import { ValueKind } from "../src/generated/flatbuffers/search-kernel/v2/value-kind.js";
import type { GrpcWebTransport } from "../src/transport/grpc-web.js";

function index(): Index {
  return Index.connect("unused", { execute: async () => new Uint8Array() } as unknown as GrpcWebTransport);
}

describe("ergonomic facade", () => {
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

  it("supports grouping expressions and catches local pagination misuse", () => {
    const expression = g.field("price").mul(agg.count()).as("revenue");
    expect(expression.label).toBe("revenue");
    expect(() => index().browse().page(2, 10).offset(20)).toThrow(/cannot follow page/);
  });

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
      .matchFeature("bm25(title)")
      .queryInput("query_embedding", "tensor<float>(x[4])")
      .definition();

    expect(text.name).toBe("title");
    expect(keyword.name).toBe("brand");
    expect(tensor.name).toBe("embedding");
    expect(rank.name).toBe("semantic");
    expect(rank.inputs).toHaveLength(1);
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

  it("keeps facet limit zero aligned with the Python facade default", () => {
    const request = index().browse().facet("brand", { limit: 0 }).build();
    const source = request.grouping[0]?.source as { root?: { operations?: Array<{ operation?: { operations?: Array<{ operation?: { value?: { value?: number } } }> } }> } };
    const each = source.root?.operations?.[1]?.operation;
    expect(each?.operations?.[0]?.operation?.value?.value).toBe(10);
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
