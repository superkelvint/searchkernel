/**
 * Public TypeScript client E2E showcase.
 *
 * Node-only: this example uses `@searchkernel/client/node`, which mirrors the
 * Python client's managed-local lifecycle by spawning searchkerneld over a
 * private Unix-domain socket. Build searchkerneld first or set
 * SEARCHKERNEL_LOCAL_HOST to its path.
 */

import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  Boolean,
  Float,
  Group,
  Index,
  Integer,
  Keyword,
  RankProfile,
  SearchKernelError,
  Tensor,
  TensorField,
  Text,
  agg,
  asc,
  desc,
  float64,
  g,
  gf,
  q,
  type DocumentInput,
  type ResultGroup,
  type SearchResults,
} from "@searchkernel/client/node";

let WORKSPACE_PATH = "";
let INDEX_PATH = "";
// SKC: update.document_crud/basic kinds=ergonomic runtime=node
// SKC: filter.textual/basic kinds=ergonomic runtime=node
// SKC: search.sorting/basic kinds=ergonomic runtime=node
// SKC: search.pagination/basic kinds=ergonomic runtime=node
// SKC: result.docsum_field_projection/basic kinds=ergonomic runtime=node
// SKC: query.nearest_neighbor/basic kinds=ergonomic runtime=node
// SKC: grouping.structured/basic kinds=ergonomic runtime=node
// SKC: grouping.aggregations/basic kinds=ergonomic runtime=node
const VECTOR_TYPE = "tensor<float>(x[8])";

function embedding(...values: number[]): Tensor {
  if (values.length !== 8) throw new TypeError("the E2E corpus uses eight-dimensional embeddings");
  return Tensor.dense(VECTOR_TYPE, values);
}

const SHOE_1_VECTOR = [0.92, 0.14, 0.05, 0.74, 0.76, 0.42, 0.20, 0.78] as const;
const SHOE_2_VECTOR = [0.74, 0.55, 0.08, 0.58, 0.42, 0.86, 0.18, 0.64] as const;
const SHOE_3_VECTOR = [0.10, 0.16, 0.78, 0.12, 0.24, 0.28, 0.92, 0.16] as const;
const SHOE_4_VECTOR = [0.82, 0.12, 0.06, 0.96, 0.72, 0.82, 0.10, 0.88] as const;
const SHOE_5_VECTOR = [0.64, 0.82, 0.07, 0.62, 0.34, 0.92, 0.16, 0.62] as const;
const BAG_1_VECTOR = [0.45, 0.25, 0.05, 0.35, 0.95, 0.15, 0.05, 0.55] as const;

function heading(title: string): void {
  console.log();
  console.log("=".repeat(80));
  console.log(title);
  console.log("=".repeat(80));
}

function assert(condition: unknown, message = "assertion failed"): asserts condition {
  if (!condition) throw new Error(message);
}

function closeTo(actual: number, expected: number, tolerance = 1e-6): boolean {
  return Math.abs(actual - expected) <= tolerance * Math.max(1, Math.abs(actual), Math.abs(expected));
}

function asNumber(value: unknown): number {
  if (typeof value !== "number") throw new TypeError(`expected number, got ${typeof value}`);
  return value;
}

function asBigInt(value: unknown): bigint {
  if (typeof value !== "bigint") throw new TypeError(`expected bigint, got ${typeof value}`);
  return value;
}

function printResults(results: SearchResults): void {
  console.log("total_hits =", results.totalHits, "relation =", results.totalHitsRelation);
  if (Object.keys(results.aggregations).length) console.log("aggregations:", results.aggregations);

  for (const hit of results.hits) {
    console.log(
      `  id=${JSON.stringify(hit.id)}`,
      `relevance=${String(hit.relevance)}`,
      "features=",
      hit.features,
      "sort_values=",
      hit.sortValues,
      hit.fields,
    );
  }

  if (results.coverage) {
    console.log("coverage:", {
      percent: results.coverage.coveragePercent,
      documents: results.coverage.documents,
      full: results.coverage.full,
      degraded: results.coverage.degraded,
    });
  }

  if (Object.keys(results.facets).length) {
    console.log("facets:");
    for (const [name, values] of Object.entries(results.facets)) {
      console.log(`  ${name}:`);
      for (const item of values) console.log(`    ${String(item.value)}: ${String(item.count)}`);
    }
  }

  if (results.groups.length) {
    console.log("groups:");
    printGroups(results.groups, 1);
  }
}

function printGroups(groups: readonly ResultGroup[], indent = 0): void {
  const prefix = "  ".repeat(indent);
  for (const group of groups) {
    console.log(
      `${prefix}- value=${String(group.value)}`,
      `label=${String(group.label)}`,
      "fields=",
      group.fields,
      "continuations=",
      group.continuations,
    );
    for (const hit of group.hits) console.log(`${prefix}    hit id=${hit.id}`, "fields=", hit.fields);
    printGroups(group.groups, indent + 1);
  }
}

function assertThrows(fn: () => unknown, message: string): void {
  try {
    fn();
  } catch {
    console.log("PASS:", message);
    return;
  }
  throw new Error(message);
}

function assertIds(results: SearchResults, expected: readonly string[]): void {
  const actual = results.hits.map((hit) => hit.id);
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
}

function groupsByValue(groups: readonly ResultGroup[]): Map<unknown, ResultGroup> {
  return new Map(groups.map((group) => [group.value, group]));
}

async function main(): Promise<void> {
  WORKSPACE_PATH = await mkdtemp(join(tmpdir(), "searchkernel-client-api-e2e-"));
  INDEX_PATH = join(WORKSPACE_PATH, "showcase-index");
  try {
    heading("1. CREATE INDEX");

    const index = await Index.create(INDEX_PATH, {
      fields: {
        title: new Text({ stored: true, searchable: true }),
        brand: new Keyword({ stored: true, filterable: true, sortable: true, facetable: true }),
        category: new Keyword({ stored: true, filterable: true, facetable: true }),
        price: new Float({ stored: true, filterable: true, sortable: true }),
        sale_price: new Float({ stored: true, filterable: true, sortable: true }),
        rating: new Integer({ stored: true, filterable: true, sortable: true }),
        inventory: new Integer({ stored: true, filterable: true, sortable: true }),
        in_stock: new Boolean({ stored: true, filterable: true }),
        embedding: new TensorField(VECTOR_TYPE, { stored: true }),
      },
      rankProfiles: [
        new RankProfile("boosted")
          .firstPhase("query(boost)")
          .matchFeature("query(boost)"),
        new RankProfile("semantic")
          .firstPhase("closeness(embedding)")
          .queryInput("query_embedding", VECTOR_TYPE)
          .matchFeature("query(query_embedding)"),
        new RankProfile("hybrid")
          .firstPhase("bm25(title) + 10 * closeness(embedding)")
          .queryInput("query_embedding", VECTOR_TYPE)
          .matchFeature("bm25(title)")
          .matchFeature("closeness(embedding)")
          .matchFeature("query(query_embedding)"),
      ],
    });

    try {
      console.log("created:", INDEX_PATH);

      heading("2. INSERT DOCUMENTS");
      const documents: DocumentInput[] = [
        { _id: "shoe-1", title: "Jumped Running Shoes for Daily Training", brand: "Acme", category: "shoes", price: float64(89), sale_price: float64(79), rating: 5n, inventory: 10n, in_stock: true, embedding: embedding(...SHOE_1_VECTOR) },
        { _id: "shoe-2", title: "Trail Running Shoes", brand: "TrailCo", category: "shoes", price: float64(129), rating: 5n, inventory: 4n, in_stock: true, embedding: embedding(...SHOE_2_VECTOR) },
        { _id: "shoe-3", title: "Budget Walking Shoes", brand: "Budget", category: "shoes", price: float64(49), rating: 3n, inventory: 25n, in_stock: true, embedding: embedding(...SHOE_3_VECTOR) },
        { _id: "shoe-4", title: "Acme Racing Shoes", brand: "Acme", category: "shoes", price: float64(159), rating: 4n, inventory: 0n, in_stock: false, embedding: embedding(...SHOE_4_VECTOR) },
        { _id: "shoe-5", title: "TrailCo Hiking Shoes", brand: "TrailCo", category: "shoes", price: float64(109), rating: 4n, inventory: 8n, in_stock: true, embedding: embedding(...SHOE_5_VECTOR) },
        { _id: "bag-1", title: "Lightweight Running Backpack", brand: "Acme", category: "bags", price: float64(69), rating: 4n, inventory: 12n, in_stock: true, embedding: embedding(...BAG_1_VECTOR) },
      ];

      for (const document of documents) {
        await index.insert(document);
        console.log("insert:", document._id);
      }
      for (let probe = 0; probe < 200; probe += 1) {
        await index.insert({
          _id: `degraded-${probe}`,
          title: "Coverage Degradation Probe",
          category: "degradation",
          price: float64(18),
          rating: 1n,
          inventory: 18n,
          in_stock: true,
          embedding: embedding(...SHOE_3_VECTOR),
        });
      }
      console.log("inserted 200 deterministic coverage probe documents");

      heading("3. GET BY ID");
      const doc = await index.get("shoe-1");
      console.log(doc);
      assert(doc !== null);
      assert(doc._id === "shoe-1");
      assert(doc.price === 89);
      const storedEmbedding = (doc.embedding as { $searchkernel_tensor: { typeSpec: string; dense: number[] } }).$searchkernel_tensor;
      assert(storedEmbedding.typeSpec === VECTOR_TYPE);
      assert(storedEmbedding.dense.length === SHOE_1_VECTOR.length);
      assert(storedEmbedding.dense.every((actual, i) => closeTo(actual, SHOE_1_VECTOR[i]!)));

      heading("4. FULL-TEXT SEARCH");
      let results = await index.search("shoes").limit(20).execute();
      printResults(results);
      assert(new Set(results.hits.map((hit) => hit.id)).size === 5);
      for (const id of ["shoe-1", "shoe-2", "shoe-3", "shoe-4", "shoe-5"]) assert(results.hits.some((hit) => hit.id === id));
      assert(results.totalHits === 5n);

      heading("5. STEMMING: 'jump' -> 'Jumped'");
      results = await index.search("jump").limit(20).execute();
      printResults(results);
      assert(results.hits.some((hit) => hit.id === "shoe-1"));
      console.log("STEMMING PASS");

      heading("6. STRING FILTER + CLAUSE-LOCAL BINDING");
      results = await index.search("shoes").filter("price <= :max", { max: 100n }).sort("price", "asc").limit(20).execute();
      printResults(results);
      assertIds(results, ["shoe-3", "shoe-1"]);

      heading("7. REPEATED FILTERS + LOCAL BINDING SCOPES");
      results = await index.search("shoes")
        .filter("price <= :value", { value: float64(120) })
        .filter("rating >= :value", { value: 4n })
        .filter("in_stock = true")
        .sort("price", "asc")
        .limit(20)
        .execute();
      printResults(results);
      assertIds(results, ["shoe-1", "shoe-5"]);

      heading("8. FILTER GRAMMAR: OR / NOT / IN / PARENTHESES");
      results = await index.search("shoes")
        .filter("(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)", {
          brands: ["Acme", "TrailCo"],
          rating: 3n,
        })
        .sort("price", "asc")
        .limit(20)
        .execute();
      printResults(results);
      assertIds(results, ["shoe-3", "shoe-1", "shoe-5", "shoe-2"]);

      heading("9. SORT + FIELD PROJECTION + HIT ACCESS");
      results = await index.search("shoes").sort("price", "desc").fields("title", "brand", "price").limit(2).execute();
      printResults(results);
      assertIds(results, ["shoe-4", "shoe-2"]);
      assert(results.hits[0]!.get("title") === "Acme Racing Shoes");
      assert(results.hits[0]!.get("brand") === "Acme");
      assert(results.hits[0]!.get("price") === 159);
      assert(!("rating" in results.hits[0]!.fields));

      heading("10. PAGINATION");
      const allSorted = await index.search("shoes").sort("price", "asc").limit(20).execute();
      const window = await index.search("shoes").sort("price", "asc").offset(1).limit(2).execute();
      const page = await index.search("shoes").sort("price", "asc").page(2, 2).execute();
      assertIds(window, allSorted.hits.slice(1, 3).map((hit) => hit.id));
      assertIds(page, allSorted.hits.slice(2, 4).map((hit) => hit.id));
      assert(window.totalHits === allSorted.totalHits && page.totalHits === allSorted.totalHits);

      heading("11. BUILDER MISUSE");
      assertThrows(() => index.search("shoes").page(2, 20).offset(20), "page() + offset() rejected");
      assertThrows(() => index.search("shoes").offset(20).page(2, 20), "offset() + page() rejected");
      assertThrows(() => index.search("shoes").page(2, 20).limit(20), "page() + limit() rejected");
      assertThrows(() => index.search("shoes").limit(-1), "negative limit rejected");
      assertThrows(() => index.search("shoes").offset(-1), "negative offset rejected");

      heading("12. FACETS");
      results = await index.search("shoes")
        .facet("brand", { name: "brands", limit: 10, sort: "count_desc" })
        .facet("category", { limit: 10, sort: "count_desc" })
        .limit(20)
        .execute();
      printResults(results);
      assert(Object.fromEntries(results.facets.brands!.map((item) => [String(item.value), item.count]))["Acme"] === 2n);
      assert(Object.fromEntries(results.facets.brands!.map((item) => [String(item.value), item.count]))["TrailCo"] === 2n);
      assert(Object.fromEntries(results.facets.brands!.map((item) => [String(item.value), item.count]))["Budget"] === 1n);
      assert(Object.fromEntries(results.facets.category!.map((item) => [String(item.value), item.count]))["shoes"] === 5n);

      heading("13. FILTERED FACET");
      results = await index.search("shoes").filter("in_stock = true").facet("brand", { limit: 10, sort: "count_desc" }).limit(20).execute();
      const filteredBrandCounts = Object.fromEntries(results.facets.brand!.map((item) => [String(item.value), item.count]));
      assert(filteredBrandCounts.TrailCo === 2n && filteredBrandCounts.Acme === 1n && filteredBrandCounts.Budget === 1n);

      heading("14. ROOT AGGREGATIONS");
      results = await index.browse()
        .filter("category = 'shoes'")
        .aggregate(agg.count().as("products"))
        .aggregate(agg.sum("price").as("sum_price"))
        .aggregate(agg.avg("price").as("avg_price"))
        .aggregate(agg.min("price").as("min_price"))
        .aggregate(agg.max("price").as("max_price"))
        .aggregate(agg.stddev("price").as("stddev_price"))
        .limit(0)
        .execute();
      printResults(results);
      assert(asBigInt(results.aggregations.products) === 5n);
      assert(closeTo(asNumber(results.aggregations.sum_price), 535));
      assert(closeTo(asNumber(results.aggregations.avg_price), 107));
      assert(closeTo(asNumber(results.aggregations.min_price), 49));
      assert(closeTo(asNumber(results.aggregations.max_price), 159));
      assert(Number.isFinite(asNumber(results.aggregations.stddev_price)) && asNumber(results.aggregations.stddev_price) > 0);

      heading("15. GROUP BY BRAND + COUNT / AVG + ORDER");
      results = await index.search("shoes").group(
        Group.by("brand")
          .limit(10)
          .precision(100)
          .orderBy(desc(agg.count()), asc(g.field("brand")))
          .output(agg.count().as("products"), agg.avg("price").as("avg_price")),
      ).execute();
      printResults(results);
      let byBrand = groupsByValue(results.groups);
      assert(asBigInt(byBrand.get("Acme")!.get("products")) === 2n);
      assert(closeTo(asNumber(byBrand.get("Acme")!.get("avg_price")), 124));
      assert(asBigInt(byBrand.get("TrailCo")!.get("products")) === 2n);
      assert(closeTo(asNumber(byBrand.get("TrailCo")!.get("avg_price")), 119));
      assert(asBigInt(byBrand.get("Budget")!.get("products")) === 1n);
      assert(results.groups.map((group) => group.value).join(",") === "Acme,TrailCo,Budget");

      heading("16. NESTED GROUPING: CATEGORY -> BRAND");
      results = await index.browse().filter("category IN (:categories)", { categories: ["shoes", "bags"] }).group(
        Group.by("category")
          .limit(10)
          .orderBy(desc(agg.count()))
          .output(agg.count().as("products"))
          .subgroup(
            Group.by("brand")
              .limit(10)
              .orderBy(desc(agg.count()), asc(g.field("brand")))
              .output(agg.count().as("products"), agg.avg("price").as("avg_price")),
          ),
      ).execute();
      const categories = groupsByValue(results.groups);
      assert(asBigInt(categories.get("shoes")!.get("products")) === 5n);
      assert(asBigInt(categories.get("bags")!.get("products")) === 1n);
      const shoeBrands = groupsByValue(categories.get("shoes")!.groups);
      assert(asBigInt(shoeBrands.get("Acme")!.get("products")) === 2n);
      assert(asBigInt(shoeBrands.get("TrailCo")!.get("products")) === 2n);
      assert(asBigInt(shoeBrands.get("Budget")!.get("products")) === 1n);

      heading("17. GROUP ALIAS + OUTPUT");
      results = await index.search("shoes").group(
        Group.by("brand")
          .limit(10)
          .alias("revenue", agg.sum("price"))
          .orderBy(desc(g.aliasRef("revenue")))
          .output(agg.count().as("products"), g.aliasRef("revenue").as("revenue")),
      ).execute();
      byBrand = groupsByValue(results.groups);
      assert(closeTo(asNumber(byBrand.get("Acme")!.get("revenue")), 248));
      assert(closeTo(asNumber(byBrand.get("TrailCo")!.get("revenue")), 238));
      assert(closeTo(asNumber(byBrand.get("Budget")!.get("revenue")), 49));

      heading("18. GROUP KEEP FILTER");
      results = await index.search("shoes").group(
        Group.by("brand").limit(10).keep(gf.regex(g.field("brand"), "^A.*")).output(agg.count().as("products")),
      ).execute();
      assert(results.groups.length === 1 && results.groups[0]!.value === "Acme");
      assert(asBigInt(results.groups[0]!.get("products")) === 2n);

      // 19/20 remain blocked/deferred for the same native reasons as Python:
      // summary aggregation Hit children and grouping continuation tokens.

      heading("21. TYPED QUERY DSL");
      results = await index.query(q.contains("title", "shoes")).ranking("default").limit(20).execute();
      assert(results.totalHits === 5n);
      results = await index.query(q.contains("title", "shoes"))
        .input("category_boost", 1.5)
        .rankProperty("showcase", "true")
        .ranking("default")
        .limit(5)
        .execute();
      assert(results.totalHits === 5n);

      heading("RANKING, SCALAR FEATURES, AND OPAQUE FEATURES");
      const ranked = await index.search("shoes").ranking("boosted").input("boost", float64(3)).limit(20).execute();
      assert(ranked.hits.length > 0);
      assert(ranked.hits.every((hit) => hit.score !== null && closeTo(asNumber(hit.features["query(boost)"]), 3)));
      const rankPropertyResults = await index.search("shoes").ranking("boosted").rankProperty("boost", "4.0").limit(20).execute();
      assert(rankPropertyResults.hits.every((hit) => hit.score !== null && closeTo(asNumber(hit.features["query(boost)"]), 4)));
      console.log("scalar ranking feature PASS");

      heading("TENSOR READBACK + NEAREST NEIGHBOR");
      const nearest = await index.query(q.nearestNeighbor("embedding", "query_embedding"))
        .ranking("semantic")
        .input("query_embedding", embedding(...SHOE_1_VECTOR))
        .limit(3)
        .execute();
      assert(nearest.hits[0]!.id === "shoe-1");
      assert(nearest.hits[0]!.score! > nearest.hits[1]!.score!);
      assert(nearest.coverage !== null);
      assert(nearest.hits[0]!.features["query(query_embedding)"] instanceof Uint8Array);
      console.log("8D tensor query input and native nearest-neighbor search PASS");

      heading("HYBRID TEXT + VECTOR SEARCH");
      const lexical = await index.query(q.contains("title", "shoes")).limit(20).execute();
      assert(!lexical.hits.some((hit) => hit.id === "bag-1"));
      const hybrid = await index.query(q.anyOf(
        q.contains("title", "shoes"),
        q.nearestNeighbor("embedding", "query_embedding"),
      ))
        .ranking("hybrid")
        .input("query_embedding", embedding(...BAG_1_VECTOR))
        .limit(20)
        .execute();
      assert(hybrid.hits.some((hit) => hit.id === "bag-1"));
      assert(hybrid.hits[0]!.id === "bag-1");
      const top = hybrid.hits[0]!;
      assert(closeTo(asNumber(top.features["bm25(title)"]), 0));
      assert(asNumber(top.features["closeness(embedding)"]) > 0.99);
      assert(closeTo(top.score!, asNumber(top.features["bm25(title)"]) + 10 * asNumber(top.features["closeness(embedding)"]), 1e-5));
      const lexicalHit = hybrid.hits.find((hit) => hit.id === "shoe-1")!;
      assert(asNumber(lexicalHit.features["bm25(title)"]) > 0);
      assert(closeTo(lexicalHit.score!, asNumber(lexicalHit.features["bm25(title)"]) + 10 * asNumber(lexicalHit.features["closeness(embedding)"]), 1e-5));
      console.log("lexical + 8D vector candidate union and hybrid rank score PASS");

      heading("COVERAGE AND DEGRADATION");
      const normal = await index.search("shoes").limit(20).execute();
      assert(normal.coverage?.full === true);
      const degraded = await index.browse()
        .filter("inventory = :value", { value: 18n })
        .sort("inventory", "asc")
        .rankProperty("vespa.matchphase.degradation.maxfiltercoverage", "0.9")
        .rankProperty("vespa.matchphase.degradation.attribute", "inventory")
        .rankProperty("vespa.matchphase.degradation.maxhits", "10")
        .limit(10)
        .execute();
      assert(degraded.hits.length > 0);
      assert(degraded.coverage?.full === false);
      assert(degraded.coverage.degraded !== null);
      assert(!!(degraded.coverage.degraded.timeout || degraded.coverage.degraded.matchPhase));
      console.log("coverage and valid degradation metadata PASS");

      heading("22. YQL + PROPERTY + PAGINATION");
      results = await index.yql("select * from sources * where title contains text(@q) order by price asc")
        .property("q", "shoes")
        .ranking("default")
        .page(1, 3)
        .execute();
      printResults(results);
      assertIds(results, ["shoe-3", "shoe-1", "shoe-5"]);
      assert(results.totalHits === 5n);

      heading("23. YQL + PROGRAMMATIC GROUPING");
      results = await index.yql("select * from sources * where title contains text(@q)")
        .property("q", "shoes")
        .group(Group.by("brand").limit(10).output(agg.count().as("products")))
        .execute();
      byBrand = groupsByValue(results.groups);
      assert(asBigInt(byBrand.get("Acme")!.get("products")) === 2n);
      assert(asBigInt(byBrand.get("TrailCo")!.get("products")) === 2n);
      assert(asBigInt(byBrand.get("Budget")!.get("products")) === 1n);

      heading("24. STRUCTURED FILTER ERROR");
      try {
        await index.search("shoes").filter("prcie <= :max", { max: 100n }).execute();
        throw new Error("unknown field filter should fail");
      } catch (error) {
        assert(error instanceof SearchKernelError);
        assert(error.details.length > 0);
        const detail = error.details[0] as { code?: string; path?: string };
        assert(detail.code === "unknown_field");
        assert(detail.path === "filters.textual[0].source");
      }
      try {
        await index.search("shoes").filter("price <= :max", { unused: 100n }).execute();
        throw new Error("missing filter binding should fail");
      } catch (error) {
        assert(error instanceof SearchKernelError);
        assert((error.details as Array<{ code?: string }>).some((detail) => detail.code === "unknown_parameter"));
      }
      try {
        await index.search("shoes").filter("price <= :max", { max: 100n, unused: 200n }).execute();
        throw new Error("unused filter binding should fail");
      } catch (error) {
        assert(error instanceof SearchKernelError);
        assert((error.details as Array<{ code?: string }>).some((detail) => detail.code === "unused_parameter"));
      }

      heading("25. PATCH BUILDER");
      await index.patch("shoe-1").set("price", float64(79)).set("rating", 4n).unset("sale_price").execute();
      let updated = await index.get("shoe-1");
      assert(updated !== null && updated.price === 79 && updated.rating === 4n && !("sale_price" in updated));
      results = await index.search("shoes").filter("price < :max", { max: 80n }).sort("price", "asc").limit(20).execute();
      assertIds(results, ["shoe-3", "shoe-1"]);

      heading("26. ADVANCED UPDATE BUILDER");
      await index.update("shoe-1").unset("sale_price").execute();
      await index.update("shoe-1").set("title", "Fast Marathon Shoes").execute();
      updated = await index.get("shoe-1");
      assert(updated?.title === "Fast Marathon Shoes");
      assert((await index.search("marathon").execute()).hits.some((hit) => hit.id === "shoe-1"));
      assert(!(await index.search("jumped").execute()).hits.some((hit) => hit.id === "shoe-1"));
      await index.update("shoe-1").set("inventory", 20n).execute();
      await index.update("shoe-1").increment("inventory", 5n).execute();
      assert((await index.get("shoe-1"))?.inventory === 25n);
      await index.update("shoe-1").decrement("inventory", 3n).execute();
      assert((await index.get("shoe-1"))?.inventory === 22n);
      await index.update("shoe-1").multiply("inventory", 2n).execute();
      assert((await index.get("shoe-1"))?.inventory === 44n);
      await index.update("shoe-1").divide("inventory", 4n).execute();
      assert((await index.get("shoe-1"))?.inventory === 11n);
      await index.update("shoe-created").set("inventory", 7n).create().execute();
      assert((await index.get("shoe-created"))?.inventory === 7n);
      try {
        await index.update("shoe-1").set("inventory", 16n).condition("inventory == 15").execute();
        throw new Error("embedded update conditions must be typed Unsupported");
      } catch (error) {
        assert(error instanceof SearchKernelError && error.code === "unsupported");
      }

      heading("27. DELETE");
      assert(await index.delete("shoe-3") === true);
      assert(await index.get("shoe-3") === null);
      results = await index.search("shoes").limit(20).execute();
      assert(!results.hits.some((hit) => hit.id === "shoe-3"));
      assert(results.totalHits === 4n);
      results = await index.search("shoes").facet("brand").limit(20).execute();
      assert(!Object.fromEntries(results.facets.brand!.map((item) => [String(item.value), item.count])).Budget);
      assert(await index.delete("shoe-3") === false);
      console.log("DELETE PASS");

      heading("28. UPSERT REPLACEMENT");
      await index.upsert({
        _id: "shoe-2",
        title: "Trail Running Shoes - Updated Edition",
        brand: "TrailCo",
        category: "shoes",
        price: float64(119),
        rating: 5n,
        inventory: 4n,
        in_stock: true,
        embedding: embedding(...SHOE_2_VECTOR),
      });
      const shoe2 = await index.get("shoe-2");
      assert(shoe2?.price === 119 && String(shoe2.title).includes("Updated Edition"));
      results = await index.search("updated").limit(20).execute();
      assert(results.hits.length === 1 && results.hits[0]!.id === "shoe-2");
    } finally {
      await index.close();
    }

    heading("29. INDEX CLOSED");
    console.log("The first managed searchkerneld instance has shut down.");

    heading("30. REOPEN PERSISTED INDEX");
    const reopened = await Index.open(INDEX_PATH);
    try {
      assert(await reopened.get("shoe-3") === null);
      const shoe1 = await reopened.get("shoe-1");
      assert(shoe1 !== null);
      assert(shoe1.price === 79);
      assert(shoe1.rating === 4n);
      assert(shoe1.inventory === 11n);
      assert(shoe1.title === "Fast Marathon Shoes");
      assert((await reopened.search("marathon").execute()).hits.some((hit) => hit.id === "shoe-1"));
      assert(!(await reopened.search("jumped").execute()).hits.some((hit) => hit.id === "shoe-1"));
      assert((await reopened.get("shoe-created"))?.inventory === 7n);
      assert(!("sale_price" in shoe1));
      const reopenedEmbedding = (shoe1.embedding as { $searchkernel_tensor: { typeSpec: string; dense: number[] } }).$searchkernel_tensor;
      assert(reopenedEmbedding.typeSpec === VECTOR_TYPE);
      assert(reopenedEmbedding.dense.every((actual, i) => closeTo(actual, SHOE_1_VECTOR[i]!)));

      const shoe2 = await reopened.get("shoe-2");
      assert(shoe2?.price === 119 && String(shoe2.title).includes("Updated Edition"));

      let results = await reopened.search("shoes").filter("price <= :max", { max: 120n }).sort("price", "asc").limit(20).execute();
      assertIds(results, ["shoe-1", "shoe-5", "shoe-2"]);

      const reopenedRanked = await reopened.search("shoes").ranking("boosted").input("boost", float64(3)).limit(20).execute();
      assert(reopenedRanked.hits.every((hit) => hit.score !== null && closeTo(asNumber(hit.features["query(boost)"]), 3)));

      const reopenedNearest = await reopened.query(q.nearestNeighbor("embedding", "query_embedding"))
        .ranking("semantic")
        .input("query_embedding", embedding(...SHOE_1_VECTOR))
        .limit(3)
        .execute();
      assert(reopenedNearest.hits[0]!.id === "shoe-1");

      heading("31. BROWSE AFTER REOPEN");
      results = await reopened.browse()
        .filter("in_stock = true")
        .sort("price", "asc")
        .facet("brand")
        .group(Group.by("category").limit(10).output(agg.count().as("products")))
        .limit(256)
        .execute();
      const browseIds = new Set(results.hits.map((hit) => hit.id));
      assert(!browseIds.has("shoe-4") && !browseIds.has("shoe-3"));
      for (const id of ["shoe-1", "shoe-2", "shoe-5", "bag-1"]) assert(browseIds.has(id));
      const categories = groupsByValue(results.groups);
      assert(asBigInt(categories.get("shoes")!.get("products")) === 3n);
      assert(asBigInt(categories.get("bags")!.get("products")) === 1n);
    } finally {
      await reopened.close();
    }

    heading("32. ALL CLIENT API E2E CHECKS PASSED");
    console.log(`
Successfully demonstrated through the public TypeScript/Node facade:

  DOCUMENTS
  ✓ create/open persistent index via managed searchkerneld
  ✓ insert / upsert / get / delete
  ✓ PatchBuilder set/unset
  ✓ UpdateBuilder set / unset / arithmetic / create
  ✓ persistence across daemon close/reopen

  SEARCH / FILTERING
  ✓ full-text search and stemming
  ✓ string filters + clause-local bindings
  ✓ sorting, projection, pagination, structured errors

  FACETS / GROUPING / AGGREGATION
  ✓ facets, filtered facets, root aggregations
  ✓ Group.by(), nested groups, aliases, keep/filter
  — summary aggregation Hit children [same native blocker as Python]
  — grouping continuation preservation [same native deferral as Python]

  QUERY / RANKING / YQL
  ✓ typed query DSL, ranking/query inputs, nearest-neighbor, hybrid
  ✓ raw YQL + properties + programmatic grouping

  RESULTS
  ✓ Hit/ResultGroup/SearchResults ergonomics
  ✓ tensor readback, feature values, coverage/degradation
`);
  } finally {
    await rm(WORKSPACE_PATH, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
