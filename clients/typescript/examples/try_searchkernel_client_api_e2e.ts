/**
 * Public TypeScript client E2E showcase.
 *
 * Node-only: this example uses `@searchkernel/client/node`, which mirrors the
 * Python client's managed-local lifecycle by spawning searchkerneld over a
 * private Unix-domain socket. Build searchkerneld first or set
 * SEARCHKERNEL_LOCAL_HOST to its path.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { paritySafe } from "./parity-json.js";
import {
  Boolean,
  Float,
  Group,
  Index,
  Integer,
  Keyword,
  PositionField,
  ArrayOfStructField,
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
  type CreateIndexOptions,
  type ValueInput,
} from "@searchkernel/client/node";

let WORKSPACE_PATH = "";
let INDEX_PATH = "";
// SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=node
// SKC: filter.textual/basic kinds=ergonomic,e2e runtime=node
// SKC: search.sorting/basic kinds=ergonomic,e2e runtime=node
// SKC: search.pagination/basic kinds=ergonomic,e2e runtime=node
// SKC: result.docsum_field_projection/basic kinds=ergonomic,e2e runtime=node
// SKC: query.nearest_neighbor/basic kinds=ergonomic,e2e runtime=node
// SKC: query.phrase/basic kinds=ergonomic,e2e runtime=node
// SKC: query.proximity/basic kinds=ergonomic,e2e runtime=node
// SKC: grouping.structured/basic kinds=ergonomic,e2e runtime=node
// SKC: grouping.aggregations/basic kinds=ergonomic,e2e runtime=node
const E2E_CORPUS: any = JSON.parse(readFileSync(resolve(process.cwd(), "../../reference/client-real-e2e-parity/corpus.json"), "utf8"));
const PARITY_CONSUMED_INTENTS = new Map<string, Record<string, unknown>>();
function trackedIntent(scenario: string, source: Record<string, any>): Record<string, any> {
  return new Proxy(source, {
    get(target, property, receiver) {
      const value = Reflect.get(target, property, receiver);
      if (typeof property === "string" && Object.prototype.hasOwnProperty.call(target, property)) {
        const consumed = PARITY_CONSUMED_INTENTS.get(scenario) ?? {};
        consumed[property] = structuredClone(value);
        PARITY_CONSUMED_INTENTS.set(scenario, consumed);
      }
      return value;
    },
  });
}
const PARITY_INTENTS = new Proxy(E2E_CORPUS.scenario_inputs as Record<string, any>, {
  get(target, property, receiver) {
    const value = Reflect.get(target, property, receiver);
    if (typeof property === "string" && Object.prototype.hasOwnProperty.call(target, property)) {
      if (!value || typeof value !== "object" || Array.isArray(value)) {
        throw new TypeError(`parity scenario intent ${property} must be an object`);
      }
      return trackedIntent(property, value);
    }
    return value;
  },
});
const VECTOR_TYPE: string = E2E_CORPUS.schema.fields.embedding.type;

function embedding(...values: number[]): Tensor {
  if (values.length !== 8) throw new TypeError("the E2E corpus uses eight-dimensional embeddings");
  return Tensor.dense(VECTOR_TYPE, values);
}

function corpusVector(documentId: string): number[] {
  const document = E2E_CORPUS.documents.find((item: any) => item._id === documentId);
  if (!document || !Array.isArray(document.embedding)) throw new Error(`missing corpus vector: ${documentId}`);
  return document.embedding;
}

const SHOE_1_VECTOR = corpusVector("shoe-1");
const SHOE_2_VECTOR = corpusVector("shoe-2");
const SHOE_3_VECTOR = corpusVector("shoe-3");
const SHOE_4_VECTOR = corpusVector("shoe-4");
const SHOE_5_VECTOR = corpusVector("shoe-5");
const BAG_1_VECTOR = corpusVector("bag-1");

interface CorpusFieldBuilder {
  definition(name: string): any;
}

const FIELD_TYPES: Record<string, new (options?: any) => CorpusFieldBuilder> = {
  text: Text,
  keyword: Keyword,
  int64: Integer,
  float64: Float,
  bool: Boolean,
  position: PositionField,
};

function corpusField(spec: any): CorpusFieldBuilder {
  const options = Object.fromEntries(Object.entries(spec).filter(([key]) => !["type", "element", "fields"].includes(key)));
  if (spec.type === "array" && spec.element?.type === "struct") {
    const members = Object.fromEntries(Object.entries<any>(spec.element.fields).map(([name, member]) => [name, corpusField(member)]));
    return new ArrayOfStructField(members as any, options);
  }
  if (spec.type.startsWith("tensor<")) return new TensorField(spec.type, options);
  const FieldType = FIELD_TYPES[spec.type];
  if (!FieldType) throw new Error(`unsupported corpus field type: ${spec.type}`);
  return new FieldType(options);
}

function fieldsFromCorpus(): CreateIndexOptions["fields"] {
  const fields: Record<string, CorpusFieldBuilder> = {};
  for (const [name, spec] of Object.entries<any>(E2E_CORPUS.schema.fields)) {
    fields[name] = corpusField(spec);
  }
  return fields;
}

function rankProfilesFromCorpus(): RankProfile[] {
  return E2E_CORPUS.schema.rank_profiles.map((spec: any) => {
    let profile = new RankProfile(spec.name).firstPhase(spec.first_phase);
    if (spec.second_phase !== undefined) profile = profile.secondPhase(spec.second_phase);
    if (spec.rerank_count !== undefined) profile = profile.rerankCount(spec.rerank_count);
    for (const [name, typeSpec] of Object.entries<string>(spec.query_inputs ?? {})) profile = profile.queryInput(name, typeSpec);
    for (const feature of spec.match_features ?? []) profile = profile.matchFeature(feature);
    return profile;
  });
}

function corpusValue(field: string, input: unknown): ValueInput {
  const type = E2E_CORPUS.schema.fields[field]?.type;
  if (type === "float64" && typeof input === "number") return float64(input);
  if (type === "int64" && (typeof input === "number" || typeof input === "string")) return BigInt(input);
  if (type?.startsWith("tensor<") && Array.isArray(input)) return embedding(...input as number[]);
  return input as ValueInput;
}

function documentInput(source: Record<string, unknown>): DocumentInput {
  return Object.fromEntries(Object.entries(source).map(([field, input]) => [field, corpusValue(field, input)])) as DocumentInput;
}

function sharedFilterBindings(source: string, bindings: Record<string, unknown>): Record<string, ValueInput> {
  const converted: Record<string, ValueInput> = {};
  for (const [name, input] of Object.entries(bindings)) {
    const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const field = new RegExp(`([A-Za-z_][\\w.]*)\\s*(?:(?:<=|>=|=|<|>)\\s*|IN\\s*\\(\\s*):${escapedName}\\b`, "i").exec(source)?.[1];
    converted[name] = field ? corpusValue(field, input) : input as ValueInput;
  }
  return converted;
}

const PARITY_OBSERVATIONS: Record<string, unknown> = {};

function typedQueryForScenario(scenario: string, intent: any): any {
  const field = intent.field;
  switch (scenario) {
    case "typed_query_term": return q.term(field, intent.value);
    case "typed_query_contains": return q.contains(field, intent.value);
    case "typed_query_phrase": return q.phrase(field, ...intent.terms);
    case "typed_query_near": return q.near(field, ...intent.terms);
    case "typed_query_ordered_near": return q.orderedNear(field, ...intent.terms);
    case "typed_query_range":
    case "typed_query_range_exclusive": {
      const fieldType = E2E_CORPUS.schema.fields[field].type;
      const bound = (input: number) => fieldType === "float64" ? float64(input) : input;
      return q.range(field, bound(intent.lower), bound(intent.upper), {
        includeLower: intent.include_lower,
        includeUpper: intent.include_upper,
      });
    }
    case "typed_query_boolean": {
      const [left, right] = intent.or_terms;
      return q.allOf(
        q.anyOf(q.contains(left.field, left.value), q.contains(right.field, right.value)),
        q.not(q.contains(intent.not_term.field, intent.not_term.value)),
      );
    }
    case "typed_query_in": return q.inValues(field, intent.values);
    case "typed_query_equals": return q.equals(field, intent.value);
    case "typed_query_fuzzy": return q.fuzzy(field, intent.value);
    case "typed_query_weak_and": return q.weakAnd(...intent.terms.map((term: any) => q.contains(term.field, term.value)));
    case "typed_query_wand": return q.wand(field, intent.items);
    case "typed_query_weighted_set": return q.weightedSet(field, intent.items);
    case "typed_query_dot_product": return q.dotProduct(field, intent.items);
    case "typed_query_prefix": return q.prefix(field, intent.value);
    case "typed_query_substring": return q.substring(field, intent.value);
    case "typed_query_suffix": return q.suffix(field, intent.value);
    case "typed_query_same_element":
      return q.sameElement(
        field,
        q.contains(intent.first_field, intent.first_value),
        q.contains(intent.last_field, intent.last_value),
      );
    case "typed_query_geo_location": return q.geoLocation(field, intent.latitude, intent.longitude, intent.radius);
    case "typed_query_geo_box": return q.geoBox(field, intent.south, intent.west, intent.north, intent.east);
    default: throw new Error("unsupported typed parity scenario: " + scenario);
  }
}

async function sharedTypedQueryObservations(index: Index): Promise<Record<string, unknown>> {
  const observations: Record<string, unknown> = {};
  for (const scenario of Object.keys(E2E_CORPUS.scenario_inputs)) {
    if (!scenario.startsWith("typed_query_")) continue;
    const input = PARITY_INTENTS[scenario];
    const capabilities = E2E_CORPUS.client_scenario_contract[scenario].typescript;
    let builder = index.query(typedQueryForScenario(scenario, input));
    const rankingProfile = input.ranking_profile;
    if (rankingProfile) builder = builder.ranking(rankingProfile);
    const results = await builder.limit(input.limit).execute();
    const resultOrder = input.result_order;
    const feature = input.feature;
    let queryResult: unknown;
    if (feature) {
      queryResult = results.hits.map((hit) => ({ id: hit.id, feature: paritySafe(hit.features[feature]) }));
    } else {
      const ids = results.hits.map((hit) => hit.id);
      if (resultOrder === "sorted_ids") ids.sort();
      queryResult = ids;
    }
    observations[scenario] = {
      observation: { intent: PARITY_CONSUMED_INTENTS.get(scenario), query_result: queryResult },
      capabilities,
    };
  }
  return observations;
}

function aggregationExpression(spec: any): any {
  switch (spec.kind) {
    case "count": return agg.count();
    case "sum": return agg.sum(spec.field);
    case "avg": return agg.avg(spec.field);
    default: throw new Error(`unsupported parity aggregation kind: ${spec.kind}`);
  }
}

function aggregationOutput(spec: any): any {
  if (spec.kind === "alias") return g.aliasRef(spec.name).as(spec.output_name);
  return aggregationExpression(spec).as(spec.name);
}

async function captureRealEngineParity(index: Index): Promise<Record<string, unknown>> {
  const typedQueryObservations = await sharedTypedQueryObservations(index);
  const intents = PARITY_INTENTS;
  const orderedSpec = intents.pagination;
  const ordered = await index.search(orderedSpec.query).sort(orderedSpec.sort[0].field, orderedSpec.sort[0].direction).limit(orderedSpec.all_limit).execute();
  const filteredSpec = intents.filtered_facets;
  const filtered = await index.search(filteredSpec.query).filter(filteredSpec.filter).sort(orderedSpec.sort[0].field, orderedSpec.sort[0].direction).limit(filteredSpec.page_limit).execute();
  const facetSpec = intents.facets;
  const faceted = await index.search(facetSpec.query).facet(facetSpec.field, { name: facetSpec.name, limit: facetSpec.limit, sort: facetSpec.sort }).limit(facetSpec.page_limit).execute();
  const rootSpec = intents.root_aggregations;
  if (rootSpec.kind !== "browse") throw new Error("root_aggregations requires browse intent");
  let rootBuilder = index.browse().filter(rootSpec.filter);
  for (const aggregationSpec of rootSpec.aggregations) rootBuilder = rootBuilder.aggregate(aggregationOutput(aggregationSpec));
  const root = await rootBuilder.limit(rootSpec.limit).execute();
  const groupedSpec = intents.grouping_aggregations;
  const groupedOrder = groupedSpec.order.map((order: any) => {
    const expression = order.kind === "count" ? agg.count() : g.field(order.field);
    return order.direction === "desc" ? desc(expression) : asc(expression);
  });
  const grouped = await index.search(groupedSpec.query).group(
    Group.by(groupedSpec.group_by).limit(groupedSpec.limit).precision(groupedSpec.precision)
      .orderBy(...groupedOrder)
      .output(...groupedSpec.outputs.map(aggregationOutput)),
  ).execute();
  const nearestIntent = PARITY_INTENTS.nearest_neighbor;
  const nearest = await index.query(q.nearestNeighbor(nearestIntent.field, nearestIntent.query_input))
    .ranking(nearestIntent.ranking_profile)
    .input(nearestIntent.query_input, embedding(...corpusVector(nearestIntent.query_input_document)))
    .limit(nearestIntent.limit).execute();
  const hybridIntent = PARITY_INTENTS.hybrid_text_vector;
  const hybrid = await index.query(q.anyOf(
    q.contains(hybridIntent.text_field, hybridIntent.text_value),
    q.nearestNeighbor(hybridIntent.vector_field, hybridIntent.query_input),
  )).ranking(hybridIntent.ranking_profile)
    .input(hybridIntent.query_input, embedding(...corpusVector(hybridIntent.query_input_document)))
    .limit(hybridIntent.limit).execute();
  const rerankedSpec = intents.two_phase_reranking;
  const reranked = await index.search(rerankedSpec.query).ranking(rerankedSpec.ranking_profile).limit(rerankedSpec.limit).execute();
  let errorCodes: string[];
  try {
    const errorSpec = intents.structured_filter_error;
    await index.search(errorSpec.query).filter(errorSpec.filter).execute();
    throw new Error("unknown field filter unexpectedly succeeded");
  } catch (error) {
    assert(
      error instanceof SearchKernelError,
      `expected SearchKernelError, received ${error instanceof Error ? `${error.name}: ${error.message}` : String(error)}`,
    );
    errorCodes = error.details.map((detail) => (detail as { code: string }).code).sort();
  }
  const document = await index.get(intents.document_crud.document_id);
  assert(document !== null);
  const tensorSpec = intents.tensor_readback;
  const tensorDocument = await index.get(tensorSpec.document_id);
  assert(tensorDocument !== null);
  const tensorValue = tensorDocument[tensorSpec.field] as { $searchkernel_tensor: { dense: number[] } };
  const fullSpec = intents.full_text_search;
  const fullText = await index.search(fullSpec.query).limit(fullSpec.limit).execute();
  const stemmingSpec = intents.stemming;
  const stemming = await index.search(stemmingSpec.query).limit(stemmingSpec.limit).execute();
  const bindingSpec = intents.text_filter_binding;
  const textBinding = await index.search(bindingSpec.query).filter(bindingSpec.filter, sharedFilterBindings(bindingSpec.filter, bindingSpec.bindings)).sort(bindingSpec.sort[0].field, bindingSpec.sort[0].direction).limit(bindingSpec.limit).execute();
  const repeatedSpec = intents.repeated_filters;
  let repeatedBuilder = index.search(repeatedSpec.query);
  for (const filter of repeatedSpec.filters) repeatedBuilder = repeatedBuilder.filter(filter.source, sharedFilterBindings(filter.source, filter.bindings));
  const repeatedFilters = await repeatedBuilder.sort(repeatedSpec.sort[0].field, repeatedSpec.sort[0].direction).limit(repeatedSpec.limit).execute();
  const booleanSpec = intents.boolean_filter_grammar;
  const booleanFilter = await index.search(booleanSpec.query).filter(booleanSpec.filter, sharedFilterBindings(booleanSpec.filter, booleanSpec.bindings))
    .sort(booleanSpec.sort[0].field, booleanSpec.sort[0].direction).limit(booleanSpec.limit).execute();
  const projectionSpec = intents.sorting_projection;
  const projection = await index.search(projectionSpec.query).sort(projectionSpec.sort[0].field, projectionSpec.sort[0].direction).fields(...projectionSpec.projection).limit(projectionSpec.limit).execute();
  const allSorted = await index.search(orderedSpec.query).sort(orderedSpec.sort[0].field, orderedSpec.sort[0].direction).limit(orderedSpec.all_limit).execute();
  const page = await index.search(orderedSpec.query).sort(orderedSpec.sort[0].field, orderedSpec.sort[0].direction).offset(orderedSpec.offset).limit(orderedSpec.limit).execute();
  const filteredFacet = await index.search(filteredSpec.query).filter(filteredSpec.filter)
    .facet(filteredSpec.field, { limit: filteredSpec.limit, sort: filteredSpec.sort }).limit(filteredSpec.page_limit).execute();
  const nestedSpec = intents.nested_grouping;
  if (nestedSpec.kind !== "browse") throw new Error("nested_grouping requires browse intent");
  const nested = await index.browse().filter(nestedSpec.filter.source, sharedFilterBindings(nestedSpec.filter.source, nestedSpec.filter.bindings)).group(
    Group.by(nestedSpec.group_by).limit(nestedSpec.limit).output(...nestedSpec.outputs.map(aggregationOutput)).subgroup(
      Group.by(nestedSpec.nested.group_by).limit(nestedSpec.nested.limit).output(...nestedSpec.nested.outputs.map(aggregationOutput)),
    ),
  ).execute();
  const aliasSpec = intents.group_alias;
  const aliasOrder = aliasSpec.order_by_alias.direction === "desc" ? desc : asc;
  let aliasGroup = Group.by(aliasSpec.group_by).limit(aliasSpec.limit);
  for (const aliasSpecification of aliasSpec.aliases) {
    aliasGroup = aliasGroup.alias(aliasSpecification.name, aggregationExpression(aliasSpecification));
  }
  const aliased = await index.search(aliasSpec.query).group(
    aliasGroup.orderBy(aliasOrder(g.aliasRef(aliasSpec.order_by_alias.name))).output(...aliasSpec.outputs.map(aggregationOutput)),
  ).execute();
  const keepSpec = intents.group_keep_filter;
  const kept = await index.search(keepSpec.query).group(
    Group.by(keepSpec.group_by).limit(keepSpec.limit).keep(gf.regex(g.field(keepSpec.keep_regex.field), keepSpec.keep_regex.pattern)).output(...keepSpec.outputs.map(aggregationOutput)),
  ).execute();
  const rankInputSpec = intents.rank_inputs;
  let rankInputBuilder = index.search(rankInputSpec.query).ranking(rankInputSpec.ranking_profile);
  for (const [key, value] of Object.entries<any>(rankInputSpec.inputs)) {
    rankInputBuilder = rankInputBuilder.input(key, rankInputSpec.input_types[key] === "float64" ? float64(value) : value);
  }
  const rankInput = await rankInputBuilder.limit(rankInputSpec.limit).execute();
  const rankPropertySpec = intents.rank_properties;
  let rankPropertyBuilder = index.search(rankPropertySpec.query).ranking(rankPropertySpec.ranking_profile);
  for (const [key, value] of Object.entries<string>(rankPropertySpec.rank_properties)) rankPropertyBuilder = rankPropertyBuilder.rankProperty(key, value);
  const rankProperty = await rankPropertyBuilder.limit(rankPropertySpec.limit).execute();
  const degradedSpec = intents.coverage_degradation;
  if (degradedSpec.kind !== "browse") throw new Error("coverage_degradation requires browse intent");
  let degradedBuilder = index.browse().filter(degradedSpec.filter.source, sharedFilterBindings(degradedSpec.filter.source, degradedSpec.filter.bindings)).sort(degradedSpec.sort[0].field, degradedSpec.sort[0].direction);
  for (const [key, value] of Object.entries<string>(degradedSpec.rank_properties)) degradedBuilder = degradedBuilder.rankProperty(key, value);
  const degraded = await degradedBuilder.limit(degradedSpec.limit).execute();
  const yqlSpec = intents.yql;
  let yqlBuilder = index.yql(yqlSpec.source);
  for (const [key, value] of Object.entries<string>(yqlSpec.bindings)) {
    yqlBuilder = yqlBuilder.property(key, value);
  }
  const yql = await yqlBuilder.ranking(yqlSpec.ranking_profile).page(yqlSpec.page, yqlSpec.page_size).execute();
  let syntaxError: SearchKernelError;
  try {
    const syntaxSpec = intents.text_filter_syntax_error;
    await index.search(syntaxSpec.query).filter(syntaxSpec.filter).execute();
    throw new Error("malformed textual filter unexpectedly succeeded");
  } catch (error) {
    assert(error instanceof SearchKernelError);
    syntaxError = error;
  }
  const scenarioIds = (result: SearchResults) => result.hits.map((hit) => hit.id);
  const groupsByBrand = (result: SearchResults) => groupingGroups(result).map((group) => ({
    brand: group.value,
    products: paritySafe(group.get("products")),
    avg_price: paritySafe(group.get("avg_price")),
  }));
  const nestedGroups = groupingGroups(nested).map((category) => ({
    category: category.value,
    products: paritySafe(category.get("products")),
    ratings: category.groups.map((group) => ({
      rating: group.value,
      products: paritySafe(group.get("products")),
      avg_price: paritySafe(group.get("avg_price")),
    })),
  }));
  const misuseSpec = intents.builder_misuse;
  let pageAndLimitRejected = false;
  try { await index.search(misuseSpec.query).page(misuseSpec.page, misuseSpec.page_size).limit(misuseSpec.limit).execute(); } catch { pageAndLimitRejected = true; }
  let pageAndOffsetRejected = false;
  try { await index.search(misuseSpec.query).page(misuseSpec.page, misuseSpec.page_size).offset(misuseSpec.page_size).execute(); } catch { pageAndOffsetRejected = true; }
  assertThrows(() => index.search(misuseSpec.query).limit(misuseSpec.negative_limit), "negative limit rejected");
  assertThrows(() => index.search(misuseSpec.query).offset(misuseSpec.negative_offset), "negative offset rejected");
  const negativeLimitRejected = true;
  const negativeOffsetRejected = true;
  const updateSpec = intents.advanced_update;
  await index.insert(documentInput({ _id: updateSpec.document_id, ...updateSpec.initial }));
  let updateBuilder = index.update(updateSpec.document_id);
  for (const [key, value] of Object.entries<any>(updateSpec.set)) updateBuilder = updateBuilder.set(key, corpusValue(key, value));
  for (const field of updateSpec.unset) updateBuilder = updateBuilder.unset(field);
  for (const [key, value] of Object.entries<any>(updateSpec.increments)) updateBuilder = updateBuilder.increment(key, corpusValue(key, value));
  await updateBuilder.execute();
  for (const [key, value] of Object.entries<any>(updateSpec.decrements)) await index.update(updateSpec.document_id).decrement(key, corpusValue(key, value)).execute();
  for (const [key, value] of Object.entries<any>(updateSpec.multipliers)) await index.update(updateSpec.document_id).multiply(key, corpusValue(key, value)).execute();
  for (const [key, value] of Object.entries<any>(updateSpec.divisors)) await index.update(updateSpec.document_id).divide(key, corpusValue(key, value)).execute();
  const advancedUpdate = await index.get(updateSpec.document_id);
  const observations: Record<string, any> = {
    document_crud: { id: document._id, title: document.title, brand: document.brand, price: document.price },
    full_text_search: { ids: scenarioIds(fullText), total: paritySafe(fullText.totalHits) },
    stemming: { ids: scenarioIds(stemming) },
    text_filter_binding: { ids: scenarioIds(textBinding) },
    repeated_filters: { ids: scenarioIds(repeatedFilters) },
    boolean_filter_grammar: { ids: scenarioIds(booleanFilter) },
    sorting_projection: { hits: projection.hits.map((hit) => ({ id: hit.id, fields: paritySafe(hit.fields) })) },
    pagination: { all_ids: scenarioIds(allSorted), offset_ids: scenarioIds(page), total: paritySafe(page.totalHits) },
    builder_misuse: {
      observation: {
        page_and_limit_rejected: pageAndLimitRejected,
        page_and_offset_rejected: pageAndOffsetRejected,
        negative_limit_rejected: negativeLimitRejected,
        negative_offset_rejected: negativeOffsetRejected,
      },
      capabilities: {
        negative_limit_rejected: { kind: "runtime_rejected" },
        negative_offset_rejected: { kind: "runtime_rejected" },
      },
    },
    sorted_search: { ids: ordered.hits.map((hit) => hit.id), total: paritySafe(ordered.totalHits) },
    filtered_search: { ids: filtered.hits.map((hit) => hit.id), total: paritySafe(filtered.totalHits) },
    facets: faceted.facets.brands!.map((item) => ({ value: String(item.value), count: paritySafe(item.count) })),
    root_aggregations: paritySafe(root.aggregations),
    advanced_update: { title: advancedUpdate!.title, inventory: paritySafe(advancedUpdate!.inventory), sale_price_present: "sale_price" in advancedUpdate! },
    grouping_aggregations: groupsByBrand(grouped),
    filtered_facets: filteredFacet.facets.brand!.map((item) => ({ value: String(item.value), count: paritySafe(item.count) })),
    nested_grouping: nestedGroups,
    group_alias: groupingGroups(aliased).map((group) => ({ value: group.value, fields: paritySafe(group.fields) })),
    group_keep_filter: groupingGroups(kept).map((group) => ({ value: group.value, fields: paritySafe(group.fields) })),
    ...typedQueryObservations,
    rank_inputs: rankInput.hits.map((hit) => ({ id: hit.id, score: hit.score, feature: paritySafe(hit.features["query(boost)"]) })),
    rank_properties: rankProperty.hits.map((hit) => ({ id: hit.id, score: hit.score, feature: paritySafe(hit.features["query(boost)"]) })),
    tensor_readback: paritySafe(tensorValue.$searchkernel_tensor.dense),
    nearest_neighbor: { observation: { intent: PARITY_CONSUMED_INTENTS.get("nearest_neighbor"), ids: nearest.hits.map((hit) => hit.id), scores: nearest.hits.map((hit) => hit.relevance) }, capabilities: {} },
    hybrid_text_vector: { observation: { intent: PARITY_CONSUMED_INTENTS.get("hybrid_text_vector"), ids: hybrid.hits.map((hit) => hit.id), top_score: hybrid.hits[0]!.score }, capabilities: {} },
    hybrid: { top_id: hybrid.hits[0]!.id, has_bag: hybrid.hits.some((hit) => hit.id === "bag-1") },
    two_phase_reranking: { ids: reranked.hits.map((hit) => hit.id) },
    coverage_degradation: {
      ids: degraded.hits.map((hit) => hit.id),
      full: degraded.coverage?.full,
      degraded: degraded.coverage?.degraded !== null,
    },
    yql: { ids: scenarioIds(yql), total: paritySafe(yql.totalHits) },
    unknown_field_error: { code: "invalid_argument", details: errorCodes },
    structured_filter_error: { code: "invalid_argument", details: errorCodes },
    text_filter_syntax_error: {
      code: "invalid_argument",
      details: syntaxError!.details.map((detail) => (detail as { code: string }).code).sort(),
    },
  };
  return observations;
}

function writeRealEngineParity(observations: Record<string, unknown>): void {
  const path = process.env.SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT;
  if (path) {
    for (const [scenario, value] of Object.entries<any>(observations)) {
      if (scenario.startsWith("typed_query_")) continue;
      const intent = PARITY_CONSUMED_INTENTS.get(scenario);
      if (intent === undefined) throw new Error(`parity scenario ${scenario} emitted an observation without consuming its shared intent`);
      const payload = value && value.observation ? value.observation : value;
      if (Array.isArray(payload)) observations[scenario] = { intent, result: value };
      else if (payload && typeof payload === "object" && !("intent" in payload)) payload.intent = intent;
      else if (!payload || typeof payload !== "object") observations[scenario] = { intent, result: value };
    }
    writeFileSync(path, JSON.stringify({ version: 1, scenarios: paritySafe(observations) }));
  }
}

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

function groupingGroups(results: SearchResults): readonly ResultGroup[] {
  const candidates = results.groups.filter((group) =>
    group.id === null
    && group.label === null
    && group.value === null
    && Object.keys(group.fields).length === 0
    && group.hits.length === 0
  );
  assert(candidates.length === 1, `expected one native grouping wrapper, got ${candidates.length}`);
  return candidates[0]!.groups;
}

async function main(): Promise<void> {
  WORKSPACE_PATH = await mkdtemp(join(tmpdir(), "searchkernel-client-api-e2e-"));
  INDEX_PATH = join(WORKSPACE_PATH, "showcase-index");
  try {
    heading("1. CREATE INDEX");

    const index = await Index.create(INDEX_PATH, {
      fields: fieldsFromCorpus(),
      rankProfiles: rankProfilesFromCorpus(),
    });

    try {
      console.log("created:", INDEX_PATH);

      heading("2. INSERT DOCUMENTS");
      const documents: DocumentInput[] = E2E_CORPUS.documents.map((item: Record<string, unknown>) => documentInput(item));

      for (const document of documents) {
        await index.insert(document);
        console.log("insert:", document._id);
      }
      const probeSpec = E2E_CORPUS.coverage_probes;
      for (let probe = 0; probe < probeSpec.count; probe += 1) {
        await index.insert(documentInput({ ...probeSpec.document, _id: `degraded-${probe}` }));
      }
      console.log(`inserted ${probeSpec.count} deterministic coverage probe documents`);

      Object.assign(PARITY_OBSERVATIONS, await captureRealEngineParity(index));
      const patchSpec = PARITY_INTENTS.patch;
      await index.insert(documentInput({ _id: patchSpec.document_id, ...patchSpec.initial }));
      let patchBuilder = index.patch(patchSpec.document_id);
      for (const [key, value] of Object.entries<any>(patchSpec.set)) patchBuilder = patchBuilder.set(key, corpusValue(key, value));
      for (const field of patchSpec.unset) patchBuilder = patchBuilder.unset(field);
      await patchBuilder.execute();
      const mutated = await index.get(patchSpec.document_id);
      assert(mutated !== null);
      PARITY_OBSERVATIONS.patch = { price: mutated.price, sale_price_present: "sale_price" in mutated };
      const deleteSpec = PARITY_INTENTS.delete;
      const parityMutationDeleted = await index.delete(deleteSpec.document_id);
      PARITY_OBSERVATIONS.delete = { deleted: parityMutationDeleted, absent_after_delete: (await index.get(deleteSpec.document_id)) === null };
      const reopenSpec = PARITY_INTENTS.reopen_persistence;
      await index.insert(documentInput({ _id: reopenSpec.marker_id, ...reopenSpec.marker }));

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
      let byBrand = groupsByValue(groupingGroups(results));
      assert(asBigInt(byBrand.get("Acme")!.get("products")) === 2n);
      assert(closeTo(asNumber(byBrand.get("Acme")!.get("avg_price")), 124));
      assert(asBigInt(byBrand.get("TrailCo")!.get("products")) === 2n);
      assert(closeTo(asNumber(byBrand.get("TrailCo")!.get("avg_price")), 119));
      assert(asBigInt(byBrand.get("Budget")!.get("products")) === 1n);
      assert(groupingGroups(results).map((group) => group.value).join(",") === "Acme,TrailCo,Budget");

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
      const categories = groupsByValue(groupingGroups(results));
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
      byBrand = groupsByValue(groupingGroups(results));
      assert(closeTo(asNumber(byBrand.get("Acme")!.get("revenue")), 248));
      assert(closeTo(asNumber(byBrand.get("TrailCo")!.get("revenue")), 238));
      assert(closeTo(asNumber(byBrand.get("Budget")!.get("revenue")), 49));

      heading("18. GROUP KEEP FILTER");
      results = await index.search("shoes").group(
        Group.by("brand").limit(10).keep(gf.regex(g.field("brand"), "^A.*")).output(agg.count().as("products")),
      ).execute();
      const keptGroups = groupingGroups(results);
      assert(keptGroups.length === 1 && keptGroups[0]!.value === "Acme");
      assert(asBigInt(keptGroups[0]!.get("products")) === 2n);

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

      heading("TWO-PHASE RANKING / SECOND-PHASE RERANK");
      const firstPhase = await index.search("candidate").ranking("first_phase_only").limit(4).execute();
      assertIds(firstPhase, ["rerank-a", "rerank-b", "rerank-c", "rerank-d"]);
      assert(firstPhase.hits.map((hit) => hit.score).join(",") === "100,90,80,70");
      const twoPhase = await index.search("candidate").ranking("two_phase").limit(4).execute();
      assertIds(twoPhase, ["rerank-b", "rerank-c", "rerank-a", "rerank-d"]);
      assert(twoPhase.hits[0]!.score === 100 && twoPhase.hits[1]!.score === 90 && twoPhase.hits[2]!.score === 1);
      assert(Number.isFinite(twoPhase.hits[3]!.score));
      assert(twoPhase.hits[3]!.score !== 1000);
      console.log("first-phase ordering and finite-window second-phase reranking PASS");

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
      byBrand = groupsByValue(groupingGroups(results));
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
      const upsertSpec = PARITY_INTENTS.upsert_replacement;
      const replacement = { ...upsertSpec.replacement };
      const vectorDocumentId = replacement.vector_document_id;
      delete replacement.vector_document_id;
      await index.upsert(documentInput({ _id: upsertSpec.document_id, ...replacement, embedding: embedding(...corpusVector(vectorDocumentId)) }));
      const shoe2 = await index.get(upsertSpec.document_id);
      assert(shoe2?.price === 119 && String(shoe2.title).includes("Updated Edition"));
      results = await index.search(upsertSpec.search_query).limit(20).execute();
      assert(results.hits.length === 1 && results.hits[0]!.id === upsertSpec.document_id);
      PARITY_OBSERVATIONS.upsert_replacement = { price: shoe2?.price, updated_title_matches: String(shoe2?.title).includes("Updated"), search_ids: results.hits.map((hit) => hit.id) };
    } finally {
      await index.close();
    }

    heading("29. INDEX CLOSED");
    console.log("The first managed searchkerneld instance has shut down.");

    heading("30. REOPEN PERSISTED INDEX");
    const reopened = await Index.open(INDEX_PATH);
    try {
      assert(await reopened.get("shoe-3") === null);
      const reopenSpec = PARITY_INTENTS.reopen_persistence;
      const upsertSpec = PARITY_INTENTS.upsert_replacement;
      const persistedParity = await reopened.get(reopenSpec.marker_id);
      assert(persistedParity !== null);
      const persistedShoe1 = await reopened.get("shoe-1");
      const persistedShoe2 = await reopened.get(upsertSpec.document_id);
      PARITY_OBSERVATIONS.reopen_persistence = {
        marker: { title: persistedParity.title, price: persistedParity.price, inventory: persistedParity.inventory },
        upserted: { updated_title_matches: String(persistedShoe2?.title).includes("Updated"), price: persistedShoe2?.price },
        deleted_absent: (await reopened.get(PARITY_INTENTS.delete.document_id)) === null,
      };
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
      const categories = groupsByValue(groupingGroups(results));
      assert(asBigInt(categories.get("shoes")!.get("products")) === 3n);
      assert(asBigInt(categories.get("bags")!.get("products")) === 1n);
    } finally {
      await reopened.close();
    }

    writeRealEngineParity(PARITY_OBSERVATIONS);

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
