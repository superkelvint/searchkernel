<?php

declare(strict_types=1);

spl_autoload_register(static function (string $class): void {
    $prefix = 'SearchKernel\\';
    if (!str_starts_with($class, $prefix)) {
        return;
    }
    $path = dirname(__DIR__) . '/src/' . substr($class, strlen($prefix)) . '.php';
    if (is_file($path)) {
        require $path;
    }
});

use SearchKernel\Agg;
use SearchKernel\Bytes;
use SearchKernel\G;
use SearchKernel\Gf;
use SearchKernel\Group;
use SearchKernel\GroupingOrder;
use SearchKernel\Index;
use SearchKernel\Query;
use SearchKernel\SearchKernelException;
use SearchKernel\Tensor;

require __DIR__ . '/searchkernel_client_api_e2e_schema.php';

$corpusPath = dirname(__DIR__, 3) . '/reference/client-real-e2e-parity/corpus.json';
$e2eCorpus = json_decode((string) file_get_contents($corpusPath), true, flags: JSON_THROW_ON_ERROR);
$parityConsumedIntents = [];

final class ConsumedIntent implements ArrayAccess
{
    public function __construct(private string $scenario, private array $source) {}

    public function offsetExists(mixed $offset): bool
    {
        return is_string($offset) && array_key_exists($offset, $this->source);
    }

    public function offsetGet(mixed $offset): mixed
    {
        global $parityConsumedIntents;
        if (!is_string($offset) || !array_key_exists($offset, $this->source)) {
            throw new OutOfBoundsException('unknown parity intent field');
        }
        $value = $this->source[$offset];
        $parityConsumedIntents[$this->scenario] ??= [];
        $parityConsumedIntents[$this->scenario][$offset] = $value;
        return $value;
    }

    public function offsetSet(mixed $offset, mixed $value): void
    {
        throw new LogicException('parity intents are immutable');
    }

    public function offsetUnset(mixed $offset): void
    {
        throw new LogicException('parity intents are immutable');
    }
}

final class ConsumedIntentMap implements ArrayAccess
{
    public function __construct(private array $source) {}

    public function offsetExists(mixed $offset): bool
    {
        return is_string($offset) && array_key_exists($offset, $this->source);
    }

    public function offsetGet(mixed $offset): mixed
    {
        if (!is_string($offset) || !array_key_exists($offset, $this->source)) {
            throw new OutOfBoundsException('unknown parity scenario intent');
        }
        $value = $this->source[$offset];
        if (!is_array($value) || array_is_list($value)) {
            throw new TypeError("parity scenario intent {$offset} must be an object");
        }
        return new ConsumedIntent($offset, $value);
    }

    public function offsetSet(mixed $offset, mixed $value): void
    {
        throw new LogicException('parity intents are immutable');
    }

    public function offsetUnset(mixed $offset): void
    {
        throw new LogicException('parity intents are immutable');
    }
}

$parityIntents = new ConsumedIntentMap($e2eCorpus['scenario_inputs']);
$vectorType = $e2eCorpus['schema']['fields']['embedding']['type'];
define('VECTOR_TYPE', $vectorType);
function normalizeCorpusFieldValue(string $name, mixed $value): mixed
{
    global $e2eCorpus;
    if (($e2eCorpus['schema']['fields'][$name]['type'] ?? null) === 'float64' && $value !== null) {
        return (float) $value;
    }
    return $value;
}

function normalizeCorpusDocument(array $document): array
{
    foreach ($document as $name => $value) {
        if (is_string($name)) {
            $document[$name] = normalizeCorpusFieldValue($name, $value);
        }
    }
    return $document;
}

function corpusVector(string $documentId): array
{
    global $e2eCorpus;
    foreach ($e2eCorpus['documents'] as $document) {
        if ($document['_id'] === $documentId && isset($document['embedding'])) {
            return $document['embedding'];
        }
    }
    throw new RuntimeException("missing corpus vector: {$documentId}");
}

define('SHOE_1_VECTOR', corpusVector('shoe-1'));
define('SHOE_2_VECTOR', corpusVector('shoe-2'));
define('SHOE_3_VECTOR', corpusVector('shoe-3'));
define('SHOE_4_VECTOR', corpusVector('shoe-4'));
define('SHOE_5_VECTOR', corpusVector('shoe-5'));
define('BAG_1_VECTOR', corpusVector('bag-1'));

function heading(string $title): void
{
    echo "\n", str_repeat('=', 80), "\n", $title, "\n", str_repeat('=', 80), "\n";
}

function check(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function same(mixed $expected, mixed $actual, string $message): void
{
    if ($expected !== $actual) {
        throw new RuntimeException(
            $message . "\nexpected: " . var_export($expected, true) . "\nactual: " . var_export($actual, true)
        );
    }
}

function closeEnough(float $expected, float $actual, string $message, float $epsilon = 1.0e-5): void
{
    if (!is_finite($expected) || !is_finite($actual) || abs($expected - $actual) > $epsilon) {
        throw new RuntimeException($message . ": expected {$expected}, got {$actual}");
    }
}

function embedding(array $values): Tensor
{
    same(8, count($values), 'E2E tensors must have eight cells');
    return Tensor::dense(VECTOR_TYPE, $values);
}

/** @return list<string> */
function ids($results): array
{
    return array_values(array_map(static fn ($hit): string => $hit->id, $results->hits));
}

/** @param list<string> $expected */
function assertIds($results, array $expected): void
{
    same($expected, ids($results), 'unexpected hit ordering');
}

/** @return array<string|int,SearchKernel\ResultGroup> */
function groupsByValue(array $groups): array
{
    $result = [];
    foreach ($groups as $group) {
        $result[(string) $group->value] = $group;
    }
    return $result;
}

/** @return list<SearchKernel\ResultGroup> */
function groupingGroups($results): array
{
    $candidates = array_values(array_filter(
        $results->groups,
        static fn ($group): bool =>
            $group->id === null
            && $group->label === null
            && $group->value === null
            && $group->fields === []
            && $group->hits === [],
    ));
    same(1, count($candidates), 'expected one native grouping wrapper');
    return $candidates[0]->groups;
}

function expectInvalid(callable $operation, string $message): void
{
    try {
        $operation();
    } catch (InvalidArgumentException) {
        echo "PASS: {$message}\n";
        return;
    }
    throw new RuntimeException($message);
}

function detailCodes(SearchKernelException $error): array
{
    return array_map(static fn (array $detail): string => $detail['code'], $error->details);
}

/** @param array<string,mixed> $spec @return array<string,mixed> */
function typedQueryForScenario(string $scenario, ConsumedIntent $intent): array
{
    $field = $intent['field'] ?? null;
    return match ($scenario) {
        'typed_query_term' => Query::term($field, $intent['value']),
        'typed_query_contains' => Query::contains($field, $intent['value']),
        'typed_query_phrase' => Query::contains($field, Query::phrase(...$intent['terms'])),
        'typed_query_near' => Query::contains($field, Query::near(...$intent['terms'])),
        'typed_query_ordered_near' => Query::contains($field, Query::onear(...$intent['terms'])),
        'typed_query_range', 'typed_query_range_exclusive' => Query::range(
            $field,
            gte: $intent['include_lower'] ? normalizeCorpusFieldValue($field, $intent['lower']) : null,
            gt: $intent['include_lower'] ? null : normalizeCorpusFieldValue($field, $intent['lower']),
            lte: $intent['include_upper'] ? normalizeCorpusFieldValue($field, $intent['upper']) : null,
            lt: $intent['include_upper'] ? null : normalizeCorpusFieldValue($field, $intent['upper']),
        ),
        'typed_query_boolean' => Query::allOf(
            Query::anyOf(
                Query::contains($intent['or_terms'][0]['field'], $intent['or_terms'][0]['value']),
                Query::contains($intent['or_terms'][1]['field'], $intent['or_terms'][1]['value']),
            ),
            Query::negate(Query::contains($intent['not_term']['field'], $intent['not_term']['value'])),
        ),
        'typed_query_in' => Query::inValues($field, array_map(
            static fn (mixed $value): mixed => normalizeCorpusFieldValue($field, $value),
            $intent['values'],
        )),
        'typed_query_equals' => Query::eq($field, normalizeCorpusFieldValue($field, $intent['value'])),
        'typed_query_fuzzy' => Query::contains($field, Query::fuzzy($intent['value'])),
        'typed_query_weak_and' => Query::weakAnd(...array_map(
            static fn (array $term): array => Query::contains($term['field'], $term['value']),
            $intent['terms'],
        )),
        'typed_query_wand' => Query::wand($field, $intent['items']),
        'typed_query_weighted_set' => Query::weightedSet($field, $intent['items']),
        'typed_query_dot_product' => Query::dotProduct($field, $intent['items']),
        'typed_query_prefix' => Query::prefix($field, $intent['value']),
        'typed_query_substring' => Query::substring($field, $intent['value']),
        'typed_query_suffix' => Query::suffix($field, $intent['value']),
        'typed_query_same_element' => Query::contains(
            $field,
            Query::sameElement(
                Query::contains($intent['first_field'], $intent['first_value']),
                Query::contains($intent['last_field'], $intent['last_value']),
            ),
        ),
        'typed_query_geo_location' => Query::geoLocation($field, $intent['latitude'], $intent['longitude'], $intent['radius']),
        'typed_query_geo_box' => Query::geoBox($field, $intent['south'], $intent['west'], $intent['north'], $intent['east']),
        default => throw new InvalidArgumentException('unsupported typed parity scenario: ' . $scenario),
    };
}

/** @param array<string,mixed> $spec */
function parityAggregationExpression(array $spec): mixed
{
    return match ($spec['kind']) {
        'count' => Agg::count(),
        'sum' => Agg::sum($spec['field']),
        'avg' => Agg::avg($spec['field']),
        default => throw new InvalidArgumentException('unsupported parity aggregation kind: ' . $spec['kind']),
    };
}

/** @param array<string,mixed> $spec */
function parityAggregationOutput(array $spec): mixed
{
    if ($spec['kind'] === 'alias') {
        return G::aliasRef($spec['name'])->as($spec['output_name']);
    }
    return parityAggregationExpression($spec)->as($spec['name']);
}

/** @return array<string,array<string,mixed>> */
function sharedTypedQueryObservations(Index $index): array
{
    global $parityIntents, $parityConsumedIntents;
    $corpus = searchkernelPhpClientApiE2eCorpus();
    $observations = [];
    foreach ($corpus['scenario_inputs'] as $scenario => $_rawInput) {
        if (!str_starts_with($scenario, 'typed_query_')) continue;
        $input = $parityIntents[$scenario];
        $builder = $index->query(typedQueryForScenario($scenario, $input));
        if (isset($input['ranking_profile'])) {
            $builder = $builder->ranking($input['ranking_profile']);
        }
        $results = $builder->limit($input['limit'])->execute();
        $queryResultOrder = (string) $input['result_order'];
        if (isset($input['feature'])) {
            $feature = $input['feature'];
            $queryResult = array_map(
                static fn ($hit): array => ['id' => $hit->id, 'feature' => $hit->matchFeatures[$feature] ?? null],
                $results->hits,
            );
        } else {
            $queryResult = ids($results);
            if ($queryResultOrder === 'sorted_ids') sort($queryResult, SORT_STRING);
        }
        $observations[$scenario] = [
            'observation' => ['intent' => $parityConsumedIntents[$scenario], 'query_result' => $queryResult],
            'capabilities' => $corpus['client_scenario_contract'][$scenario]['php'],
        ];
    }
    return $observations;
}

function captureRealEngineParity(Index $index): array
{
    global $parityIntents, $parityConsumedIntents;
    $corpus = searchkernelPhpClientApiE2eCorpus();
    $typedObservations = sharedTypedQueryObservations($index);
    $intents = $parityIntents;
    $orderedSpec = $intents['pagination'];
    $ordered = $index->search($orderedSpec['query'])->sort($orderedSpec['sort'][0]['field'], $orderedSpec['sort'][0]['direction'])->limit($orderedSpec['all_limit'])->execute();
    $filteredSpec = $intents['filtered_facets'];
    $filtered = $index->search($filteredSpec['query'])->filter($filteredSpec['filter'])->sort($orderedSpec['sort'][0]['field'], $orderedSpec['sort'][0]['direction'])->limit($filteredSpec['page_limit'])->execute();
    $facetSpec = $intents['facets'];
    $faceted = $index->search($facetSpec['query'])->facet($facetSpec['field'], name: $facetSpec['name'], limit: $facetSpec['limit'], sort: $facetSpec['sort'])->limit($facetSpec['page_limit'])->execute();
    $rootSpec = $intents['root_aggregations'];
    if ($rootSpec['kind'] !== 'browse') throw new RuntimeException('root_aggregations requires browse intent');
    $rootBuilder = $index->browse()->filter($rootSpec['filter']);
    foreach ($rootSpec['aggregations'] as $aggregationSpec) {
        $rootBuilder = $rootBuilder->aggregate(parityAggregationOutput($aggregationSpec));
    }
    $root = $rootBuilder->limit($rootSpec['limit'])->execute();
    $groupedSpec = $intents['grouping_aggregations'];
    $groupOrder = array_map(static function (array $order): mixed {
        $expression = $order['kind'] === 'count' ? Agg::count() : G::field($order['field']);
        return $order['direction'] === 'desc' ? GroupingOrder::desc($expression) : GroupingOrder::asc($expression);
    }, $groupedSpec['order']);
    $grouped = $index->search($groupedSpec['query'])->group(
        Group::by($groupedSpec['group_by'])->limit($groupedSpec['limit'])->precision($groupedSpec['precision'])
            ->orderBy(...$groupOrder)
            ->output(...array_map(parityAggregationOutput(...), $groupedSpec['outputs']))
    )->execute();
    $nearestIntent = $intents['nearest_neighbor'];
    $nearest = $index->query(Query::nearestNeighbor($nearestIntent['field'], $nearestIntent['query_input']))
        ->ranking($nearestIntent['ranking_profile'])
        ->input($nearestIntent['query_input'], embedding(corpusVector($nearestIntent['query_input_document'])))
        ->limit($nearestIntent['limit'])->execute();
    $hybridIntent = $intents['hybrid_text_vector'];
    $hybrid = $index->query(Query::anyOf(
        Query::contains($hybridIntent['text_field'], $hybridIntent['text_value']),
        Query::nearestNeighbor($hybridIntent['vector_field'], $hybridIntent['query_input'])
    ))->ranking($hybridIntent['ranking_profile'])
        ->input($hybridIntent['query_input'], embedding(corpusVector($hybridIntent['query_input_document'])))
        ->limit($hybridIntent['limit'])->execute();
    $rerankedSpec = $intents['two_phase_reranking'];
    $reranked = $index->search($rerankedSpec['query'])->ranking($rerankedSpec['ranking_profile'])->limit($rerankedSpec['limit'])->execute();
    try {
        $errorSpec = $intents['structured_filter_error'];
        $index->search($errorSpec['query'])->filter($errorSpec['filter'])->execute();
        throw new RuntimeException('unknown field filter unexpectedly succeeded');
    } catch (SearchKernelException $error) {
        same('InvalidArgument', $error->errorCode, 'portable invalid-argument error code');
        $errorCodes = detailCodes($error);
        sort($errorCodes);
    }

    $document = $index->get($intents['document_crud']['document_id']);
    $tensorSpec = $intents['tensor_readback'];
    $tensorDocument = $index->get($tensorSpec['document_id']);
    check($tensorDocument !== null, 'tensor readback document is missing');
    $tensorValue = $tensorDocument[$tensorSpec['field']];
    $fullSpec = $intents['full_text_search'];
    $fullText = $index->search($fullSpec['query'])->limit($fullSpec['limit'])->execute();
    $stemmingSpec = $intents['stemming'];
    $stemming = $index->search($stemmingSpec['query'])->limit($stemmingSpec['limit'])->execute();
    $bindingSpec = $intents['text_filter_binding'];
    $textBinding = $index->search($bindingSpec['query'])->filter($bindingSpec['filter'], $bindingSpec['bindings'])->sort($bindingSpec['sort'][0]['field'], $bindingSpec['sort'][0]['direction'])->limit($bindingSpec['limit'])->execute();
    $repeatedSpec = $intents['repeated_filters'];
    $repeatedBuilder = $index->search($repeatedSpec['query']);
    foreach ($repeatedSpec['filters'] as $filterSpec) $repeatedBuilder = $repeatedBuilder->filter($filterSpec['source'], $filterSpec['bindings']);
    $repeated = $repeatedBuilder->sort($repeatedSpec['sort'][0]['field'], $repeatedSpec['sort'][0]['direction'])->limit($repeatedSpec['limit'])->execute();
    $booleanSpec = $intents['boolean_filter_grammar'];
    $booleanFilter = $index->search($booleanSpec['query'])->filter($booleanSpec['filter'], $booleanSpec['bindings'])->sort($booleanSpec['sort'][0]['field'], $booleanSpec['sort'][0]['direction'])->limit($booleanSpec['limit'])->execute();
    $projectionSpec = $intents['sorting_projection'];
    $projection = $index->search($projectionSpec['query'])->sort($projectionSpec['sort'][0]['field'], $projectionSpec['sort'][0]['direction'])->fields(...$projectionSpec['projection'])->limit($projectionSpec['limit'])->execute();
    $allSorted = $index->search($orderedSpec['query'])->sort($orderedSpec['sort'][0]['field'], $orderedSpec['sort'][0]['direction'])->limit($orderedSpec['all_limit'])->execute();
    $page = $index->search($orderedSpec['query'])->sort($orderedSpec['sort'][0]['field'], $orderedSpec['sort'][0]['direction'])->offset($orderedSpec['offset'])->limit($orderedSpec['limit'])->execute();
    $filteredFacetBuilder = $index->search($filteredSpec['query'])->filter($filteredSpec['filter'])->facet($filteredSpec['field'], limit: $filteredSpec['limit'], sort: $filteredSpec['sort'])->limit($filteredSpec['page_limit']);
    $filteredFacet = $filteredFacetBuilder->execute();
    $nestedSpec = $intents['nested_grouping'];
    if ($nestedSpec['kind'] !== 'browse') throw new RuntimeException('nested_grouping requires browse intent');
    $nested = $index->browse()->filter($nestedSpec['filter']['source'], $nestedSpec['filter']['bindings'])->group(
        Group::by($nestedSpec['group_by'])->limit($nestedSpec['limit'])->output(...array_map(parityAggregationOutput(...), $nestedSpec['outputs']))
            ->subgroup(Group::by($nestedSpec['nested']['group_by'])->limit($nestedSpec['nested']['limit'])->output(...array_map(parityAggregationOutput(...), $nestedSpec['nested']['outputs'])))
    )->execute();
    $aliasSpec = $intents['group_alias'];
    $aliasExpression = G::aliasRef($aliasSpec['order_by_alias']['name']);
    $aliasOrder = $aliasSpec['order_by_alias']['direction'] === 'desc' ? GroupingOrder::desc($aliasExpression) : GroupingOrder::asc($aliasExpression);
    $aliasGroup = Group::by($aliasSpec['group_by'])->limit($aliasSpec['limit']);
    foreach ($aliasSpec['aliases'] as $aliasSpecification) {
        $aliasGroup = $aliasGroup->alias($aliasSpecification['name'], parityAggregationExpression($aliasSpecification));
    }
    $aliased = $index->search($aliasSpec['query'])->group(
        $aliasGroup->orderBy($aliasOrder)
            ->output(...array_map(parityAggregationOutput(...), $aliasSpec['outputs']))
    )->execute();
    $keepSpec = $intents['group_keep_filter'];
    $kept = $index->search($keepSpec['query'])->group(
        Group::by($keepSpec['group_by'])->limit($keepSpec['limit'])->keep(Gf::regex(G::field($keepSpec['keep_regex']['field']), $keepSpec['keep_regex']['pattern']))->output(...array_map(parityAggregationOutput(...), $keepSpec['outputs']))
    )->execute();
    $typed = $index->query(typedQueryForScenario('typed_query_contains', $intents['typed_query_contains']))->ranking('default')->limit($fullSpec['limit'])->execute();
    $rankInputSpec = $intents['rank_inputs'];
    $rankInputBuilder = $index->search($rankInputSpec['query'])->ranking($rankInputSpec['ranking_profile']);
    foreach ($rankInputSpec['inputs'] as $key => $value) {
        if (($rankInputSpec['input_types'][$key] ?? null) === 'float64') $value = (float) $value;
        $rankInputBuilder = $rankInputBuilder->input($key, $value);
    }
    $rankInput = $rankInputBuilder->limit($rankInputSpec['limit'])->execute();
    $rankPropertySpec = $intents['rank_properties'];
    $rankPropertyBuilder = $index->search($rankPropertySpec['query'])->ranking($rankPropertySpec['ranking_profile']);
    foreach ($rankPropertySpec['rank_properties'] as $key => $value) $rankPropertyBuilder = $rankPropertyBuilder->rankProperty($key, $value);
    $rankProperty = $rankPropertyBuilder->limit($rankPropertySpec['limit'])->execute();
    $degradedSpec = $intents['coverage_degradation'];
    if ($degradedSpec['kind'] !== 'browse') throw new RuntimeException('coverage_degradation requires browse intent');
    $degradedBuilder = $index->browse()->filter($degradedSpec['filter']['source'], $degradedSpec['filter']['bindings'])->sort($degradedSpec['sort'][0]['field'], $degradedSpec['sort'][0]['direction']);
    foreach ($degradedSpec['rank_properties'] as $key => $value) $degradedBuilder = $degradedBuilder->rankProperty($key, $value);
    $degraded = $degradedBuilder->limit($degradedSpec['limit'])->execute();
    $yqlSpec = $intents['yql'];
    $yqlBuilder = $index->yql($yqlSpec['source']);
    foreach ($yqlSpec['bindings'] as $key => $value) $yqlBuilder = $yqlBuilder->property($key, $value);
    $yql = $yqlBuilder->ranking($yqlSpec['ranking_profile'])->page($yqlSpec['page'], $yqlSpec['page_size'])->execute();
    try {
        $syntaxSpec = $intents['text_filter_syntax_error'];
        $index->search($syntaxSpec['query'])->filter($syntaxSpec['filter'])->execute();
        throw new RuntimeException('malformed textual filter unexpectedly succeeded');
    } catch (SearchKernelException $error) {
        $syntaxError = ['code' => strtolower($error->errorCode), 'details' => detailCodes($error)];
        sort($syntaxError['details']);
    }

    $groups = groupingGroups($grouped);
    $updateSpec = $intents['advanced_update'];
    $index->insert(['_id' => $updateSpec['document_id'], ...normalizeCorpusDocument($updateSpec['initial'])]);
    $updateBuilder = $index->update($updateSpec['document_id']);
    foreach ($updateSpec['set'] as $field => $value) $updateBuilder = $updateBuilder->set($field, normalizeCorpusFieldValue($field, $value));
    foreach ($updateSpec['unset'] as $field) $updateBuilder = $updateBuilder->unset($field);
    foreach ($updateSpec['increments'] as $field => $value) $updateBuilder = $updateBuilder->increment($field, normalizeCorpusFieldValue($field, $value));
    $updateBuilder->execute();
    foreach ($updateSpec['decrements'] as $field => $value) $index->update($updateSpec['document_id'])->decrement($field, normalizeCorpusFieldValue($field, $value))->execute();
    foreach ($updateSpec['multipliers'] as $field => $value) $index->update($updateSpec['document_id'])->multiply($field, normalizeCorpusFieldValue($field, $value))->execute();
    foreach ($updateSpec['divisors'] as $field => $value) $index->update($updateSpec['document_id'])->divide($field, normalizeCorpusFieldValue($field, $value))->execute();
    $advancedUpdate = $index->get($updateSpec['document_id']);
    $misuseSpec = $intents['builder_misuse'];
    $observations = [
        'document_crud' => ['id' => $document['_id'], 'title' => $document['title'], 'brand' => $document['brand'], 'price' => $document['price']],
        'full_text_search' => ['ids' => ids($fullText), 'total' => $fullText->totalHits],
        'stemming' => ['ids' => ids($stemming)],
        'text_filter_binding' => ['ids' => ids($textBinding)],
        'repeated_filters' => ['ids' => ids($repeated)],
        'boolean_filter_grammar' => ['ids' => ids($booleanFilter)],
        'sorting_projection' => ['hits' => array_map(static fn ($hit): array => ['id' => $hit->id, 'fields' => $hit->fields], $projection->hits)],
        'pagination' => ['all_ids' => ids($allSorted), 'offset_ids' => ids($page), 'total' => $page->totalHits],
        'sorted_search' => ['ids' => ids($ordered), 'total' => $ordered->totalHits],
        'filtered_search' => ['ids' => ids($filtered), 'total' => $filtered->totalHits],
        'facets' => array_values(array_map(
            static fn (array $item): array => ['value' => (string) $item['value'], 'count' => (int) $item['count']],
            $faceted->facets['brands'],
        )),
        'root_aggregations' => $root->aggregations,
        'advanced_update' => ['title' => $advancedUpdate['title'], 'inventory' => $advancedUpdate['inventory'], 'sale_price_present' => array_key_exists('sale_price', $advancedUpdate)],
        'builder_misuse' => (function () use ($index, $misuseSpec): array {
            expectInvalid(static fn () => $index->search($misuseSpec['query'])->page($misuseSpec['page'], $misuseSpec['page_size'])->limit($misuseSpec['limit']), 'page plus limit rejected');
            expectInvalid(static fn () => $index->search($misuseSpec['query'])->page($misuseSpec['page'], $misuseSpec['page_size'])->offset($misuseSpec['page_size']), 'page plus offset rejected');
            expectInvalid(static fn () => $index->search($misuseSpec['query'])->limit($misuseSpec['negative_limit']), 'negative limit rejected');
            expectInvalid(static fn () => $index->search($misuseSpec['query'])->offset($misuseSpec['negative_offset']), 'negative offset rejected');
            return [
                'observation' => [
                    'page_and_limit_rejected' => true,
                    'page_and_offset_rejected' => true,
                    'negative_limit_rejected' => true,
                    'negative_offset_rejected' => true,
                ],
                'capabilities' => [
                    'negative_limit_rejected' => ['kind' => 'runtime_rejected'],
                    'negative_offset_rejected' => ['kind' => 'runtime_rejected'],
                ],
            ];
        })(),
        'grouping_aggregations' => array_values(array_map(static fn ($group): array => [
            'brand' => $group->value,
            'products' => $group->fields['products'],
            'avg_price' => $group->fields['avg_price'],
        ], $groups)),
        'filtered_facets' => array_values(array_map(
            static fn (array $item): array => ['value' => (string) $item['value'], 'count' => (int) $item['count']],
            $filteredFacet->facets['brand'],
        )),
        'nested_grouping' => array_values(array_map(static fn ($group): array => [
            'category' => $group->value,
            'products' => $group->fields['products'],
            'ratings' => array_map(static fn ($rating): array => ['rating' => $rating->value, 'products' => $rating->fields['products'], 'avg_price' => $rating->fields['avg_price']], $group->groups),
        ], groupingGroups($nested))),
        'group_alias' => array_values(array_map(static fn ($group): array => ['value' => $group->value, 'fields' => $group->fields], groupingGroups($aliased))),
        'group_keep_filter' => array_values(array_map(static fn ($group): array => ['value' => $group->value, 'fields' => $group->fields], groupingGroups($kept))),
        ...$typedObservations,
        'rank_inputs' => array_values(array_map(static fn ($hit): array => ['id' => $hit->id, 'score' => $hit->score(), 'feature' => $hit->matchFeatures['query(boost)'] ?? null], $rankInput->hits)),
        'rank_properties' => array_values(array_map(static fn ($hit): array => ['id' => $hit->id, 'score' => $hit->score(), 'feature' => $hit->matchFeatures['query(boost)'] ?? null], $rankProperty->hits)),
        'tensor_readback' => array_values($tensorValue->payload['values']),
        'nearest_neighbor' => ['observation' => ['intent' => $parityConsumedIntents['nearest_neighbor'], 'ids' => ids($nearest), 'scores' => array_map(static fn ($hit): float => (float) $hit->relevance, $nearest->hits)], 'capabilities' => (object) []],
        'hybrid_text_vector' => ['observation' => ['intent' => $parityConsumedIntents['hybrid_text_vector'], 'ids' => ids($hybrid), 'top_score' => $hybrid->hits[0]->score()], 'capabilities' => (object) []],
        'hybrid' => ['top_id' => $hybrid->hits[0]->id, 'has_bag' => in_array('bag-1', ids($hybrid), true)],
        'two_phase_reranking' => ['ids' => ids($reranked)],
        'coverage_degradation' => ['ids' => ids($degraded), 'full' => $degraded->coverage->full, 'degraded' => $degraded->coverage->degraded !== null],
        'yql' => ['ids' => ids($yql), 'total' => $yql->totalHits],
        'unknown_field_error' => ['code' => 'invalid_argument', 'details' => $errorCodes],
        'structured_filter_error' => ['code' => 'invalid_argument', 'details' => $errorCodes],
        'text_filter_syntax_error' => ['code' => 'invalid_argument', 'details' => $syntaxError['details']],
    ];
    return $observations;
}

function preserveEmptyParityObjects(mixed &$value): void
{
    if (!is_array($value)) {
        return;
    }
    foreach ($value as $key => &$child) {
        if (in_array($key, ['bindings', 'capabilities'], true) && $child === []) {
            $child = (object) [];
        } else {
            preserveEmptyParityObjects($child);
        }
    }
    unset($child);
}

function writeRealEngineParity(array $scenarios): void
{
    $path = getenv('SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT');
    if ($path === false || $path === '') {
        return;
    }
    global $parityConsumedIntents;
    $intentAliases = [
        'sorted_search' => 'pagination',
        'filtered_search' => 'filtered_facets',
        'hybrid' => 'hybrid_text_vector',
        'unknown_field_error' => 'structured_filter_error',
    ];
    foreach ($scenarios as $scenario => &$value) {
        if (str_starts_with($scenario, 'typed_query_')) {
            continue;
        }
        $intentName = $intentAliases[$scenario] ?? $scenario;
        if (!array_key_exists($intentName, $parityConsumedIntents)) {
            throw new RuntimeException("parity scenario {$scenario} emitted an observation without consuming its shared intent");
        }
        $intent = $parityConsumedIntents[$intentName];
        if (isset($value['observation']) && is_array($value['observation'])) {
            $value['observation']['intent'] ??= $intent;
        } elseif (is_array($value) && !array_is_list($value)) {
            $value['intent'] ??= $intent;
        } else {
            $value = ['intent' => $intent, 'result' => $value];
        }
    }
    unset($value);
    $observations = ['version' => 1, 'scenarios' => $scenarios];
    if (is_file($path)) {
        $previous = json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);
        $observations['scenarios'] = array_replace($previous['scenarios'] ?? [], $scenarios);
    }
    preserveEmptyParityObjects($observations['scenarios']);
    file_put_contents($path, json_encode($observations, JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION));
}

$url = getenv('SEARCHKERNEL_HTTP_URL') ?: 'http://127.0.0.1:50052';
$target = getenv('SEARCHKERNEL_E2E_TARGET') ?: 'php-e2e';
$phase = $argv[1] ?? 'initial';
if (!in_array($phase, ['initial', 'reopen'], true)) {
    throw new InvalidArgumentException('expected initial or reopen phase');
}
$index = $phase === 'initial'
    ? Index::create($url, $target, searchkernelPhpClientApiE2eDefinition($e2eCorpus))
    : Index::open($url, $target);

if ($phase === 'initial') {
    $firstHandle = Index::open($url, $target);
    $secondHandle = Index::open($url, $target);
    $firstHandle->close();
    check(
        $secondHandle->describe() === $index->describe(),
        'closing one PHP handle leaves an already-open sibling usable'
    );
    $secondHandle->close();

    $otherTarget = $target . '-other';
    $other = Index::create(
        $url,
        $otherTarget,
        searchkernelPhpClientApiE2eDefinition($e2eCorpus),
    );
    $other->upsert([
        '_id' => 'php-isolation',
        'title' => 'Isolated PHP index',
        'category' => 'other',
        'price' => 1.0,
        'rating' => 1,
        'inventory' => 1,
        'in_stock' => true,
    ]);
    check($other->get('php-isolation') !== null, 'second PHP logical index reads its document');
    same(null, $index->get('php-isolation'), 'PHP logical indexes remain isolated on one daemon');
    $other->close();
}

if ($phase === 'reopen') {
    heading('30. REOPEN PERSISTED INDEX');

    same(null, $index->get('shoe-3'), 'deleted document remains deleted');
    $reopenSpec = $parityIntents['reopen_persistence'];
    $reopenMarkerIntent = $reopenSpec['marker'];
    $upsertSpec = $parityIntents['upsert_replacement'];
    $persisted = $index->get($reopenSpec['marker_id']);
    check($persisted !== null, 'parity-reopen survives daemon and index reopen');
    same($reopenMarkerIntent['title'], $persisted['title'], 'reopen marker title matches the shared scenario intent');
    $persistedShoe2 = $index->get($upsertSpec['document_id']);
    writeRealEngineParity(['reopen_persistence' => [
        'marker' => ['title' => $persisted['title'], 'price' => $persisted['price'], 'inventory' => $persisted['inventory']],
        'upserted' => ['updated_title_matches' => str_contains($persistedShoe2['title'], 'Updated'), 'price' => $persistedShoe2['price']],
        'deleted_absent' => $index->get($parityIntents['delete']['document_id']) === null,
    ]]);

    $shoe1 = $index->get('shoe-1');
    check($shoe1 !== null, 'shoe-1 survives reopen');
    same(79.0, $shoe1['price'], 'patched price survives reopen');
    same(4, $shoe1['rating'], 'patched rating survives reopen');
    same(11, $shoe1['inventory'], 'advanced arithmetic survives reopen');
    same('Fast Marathon Shoes', $shoe1['title'], 'text update survives reopen');
    check(in_array('shoe-1', ids($index->search('marathon')->execute()), true), 'new terms survive reopen');
    check(!in_array('shoe-1', ids($index->search('jumped')->execute()), true), 'old terms stay removed');
    same(7, $index->get('shoe-created')['inventory'], 'create update survives reopen');
    check(!array_key_exists('sale_price', $shoe1), 'unset survives reopen');

    $tensor = $shoe1['embedding'];
    check($tensor instanceof Tensor, 'tensor readback remains typed after reopen');
    same(VECTOR_TYPE, $tensor->typeSpec, 'tensor type survives reopen');
    foreach (SHOE_1_VECTOR as $i => $expected) {
        closeEnough($expected, $tensor->payload['values'][$i], 'tensor cell survives reopen');
    }

    $shoe2 = $index->get('shoe-2');
    same(119.0, $shoe2['price'], 'upsert replacement survives reopen');
    check(str_contains($shoe2['title'], 'Updated Edition'), 'upsert title survives reopen');

    $results = $index->search('shoes')
        ->filter('price <= :max', ['max' => 120])
        ->sort('price', 'asc')
        ->limit(20)
        ->execute();
    assertIds($results, ['shoe-1', 'shoe-5', 'shoe-2']);

    $ranked = $index->search('shoes')
        ->ranking('boosted')
        ->input('boost', 3.0)
        ->limit(20)
        ->execute();
    check($ranked->hits !== [], 'ranked hits survive reopen');
    foreach ($ranked->hits as $hit) {
        check($hit->score() !== null, 'rank score survives reopen');
        closeEnough(3.0, (float) $hit->matchFeatures['query(boost)'], 'rank feature survives reopen');
    }

    $nearest = $index->query(Query::nearestNeighbor('embedding', 'query_embedding'))
        ->ranking('semantic')
        ->input('query_embedding', embedding(SHOE_1_VECTOR))
        ->limit(3)
        ->execute();
    same('shoe-1', $nearest->hits[0]->id, 'nearest-neighbor survives reopen');

    heading('31. BROWSE AFTER REOPEN');
    $results = $index->browse()
        ->filter('in_stock = true')
        ->sort('price', 'asc')
        ->facet('brand')
        ->group(
            Group::by('category')
                ->limit(10)
                ->output(Agg::count()->as('products'))
        )
        ->limit(256)
        ->execute();

    $browseIds = ids($results);
    check(!in_array('shoe-4', $browseIds, true), 'out-of-stock item excluded');
    check(!in_array('shoe-3', $browseIds, true), 'deleted item excluded');
    foreach (['shoe-1', 'shoe-2', 'shoe-5', 'bag-1'] as $id) {
        check(in_array($id, $browseIds, true), "browse contains {$id}");
    }

    $categories = groupsByValue(groupingGroups($results));
    same(3, $categories['shoes']->fields['products'], 'shoe grouping survives reopen');
    same(1, $categories['bags']->fields['products'], 'bag grouping survives reopen');

    heading('32. ALL PHP CLIENT API E2E REOPEN CHECKS PASSED');
    exit(0);
}

heading('1. INDEX SCHEMA + RANK PROFILES PREPARED BY CANONICAL HARNESS');
echo "PHP executes the same product corpus through the HTTP/FlatBuffers-JSON edge.\n";

heading('2. INSERT DOCUMENTS');
$documents = array_map(static function (array $document): array {
    $document = normalizeCorpusDocument($document);
    if (isset($document['embedding']) && is_array($document['embedding'])) {
        $document['embedding'] = embedding($document['embedding']);
    }
    return $document;
}, $e2eCorpus['documents']);
foreach ($documents as $document) {
    same($document['_id'], $index->insert($document), 'insert returns document id');
}
$probeSpec = $e2eCorpus['coverage_probes'];
for ($probe = 0; $probe < $probeSpec['count']; $probe++) {
    $document = $probeSpec['document'];
    $document = normalizeCorpusDocument($document);
    $document['_id'] = "degraded-{$probe}";
    $document['embedding'] = embedding($document['embedding']);
    $index->insert($document);
}

$parity = captureRealEngineParity($index);
$patchSpec = $parityIntents['patch'];
$index->insert(['_id' => $patchSpec['document_id'], ...normalizeCorpusDocument($patchSpec['initial'])]);
$patchBuilder = $index->patch($patchSpec['document_id']);
foreach ($patchSpec['set'] as $field => $value) $patchBuilder = $patchBuilder->set($field, normalizeCorpusFieldValue($field, $value));
foreach ($patchSpec['unset'] as $field) $patchBuilder = $patchBuilder->unset($field);
$patchBuilder->execute();
$mutated = $index->get($patchSpec['document_id']);
$parity['patch'] = ['price' => $mutated['price'], 'sale_price_present' => array_key_exists('sale_price', $mutated)];
$deleteSpec = $parityIntents['delete'];
$parityMutationDeleted = $index->delete($deleteSpec['document_id']);
$parity['delete'] = ['deleted' => $parityMutationDeleted, 'absent_after_delete' => $index->get($deleteSpec['document_id']) === null];
$reopenSpec = $parityIntents['reopen_persistence'];
$index->insert(['_id' => $reopenSpec['marker_id'], ...normalizeCorpusDocument($reopenSpec['marker'])]);

heading('3. GET BY ID');
$doc = $index->get('shoe-1');
check($doc !== null, 'shoe-1 exists');
same('shoe-1', $doc['_id'], 'id round trip');
same(89.0, $doc['price'], 'price round trip');
check($doc['embedding'] instanceof Tensor, 'stored tensor decodes to Tensor');
same(VECTOR_TYPE, $doc['embedding']->typeSpec, 'stored tensor type');
same(8, count($doc['embedding']->payload['values']), 'stored tensor cell count');
foreach (SHOE_1_VECTOR as $i => $expected) {
    closeEnough($expected, $doc['embedding']->payload['values'][$i], 'stored tensor cell');
}

heading('4. FULL-TEXT SEARCH');
$results = $index->search('shoes')->searchField('title')->limit(20)->execute();
$actual = ids($results);
sort($actual);
$expected = ['shoe-1', 'shoe-2', 'shoe-3', 'shoe-4', 'shoe-5'];
sort($expected);
same($expected, $actual, 'full-text shoes corpus');
same(5, $results->totalHits, 'total hits');

$highlighted = $index->search('jump')
    ->summary('default')
    ->fields('category')
    ->highlight('title')
    ->execute();
$highlightedHit = current(array_values(array_filter($highlighted->hits, static fn ($hit): bool => $hit->id === 'shoe-1')));
same(['category' => 'shoes'], $highlightedHit->fields, 'projection with highlight');
$fragment = $highlightedHit->highlights[0]['fragments'][0];
same('Jumped Running Shoes for Daily Training', $fragment['text'], 'highlight text');
[$start, $end] = $fragment['matches'][0];
same('Jumped', substr($fragment['text'], $start, $end - $start), 'stemmed highlight span');

heading("5. STEMMING: 'jump' -> 'Jumped'");
check(in_array('shoe-1', ids($index->search('jump')->execute()), true), 'stemming representative');

heading('6. STRING FILTER + CLAUSE-LOCAL BINDING');
$results = $index->search('shoes')
    ->filter('price <= :max', ['max' => 100])
    ->sort('price', 'asc')
    ->limit(20)
    ->execute();
assertIds($results, ['shoe-3', 'shoe-1']);

heading('7. REPEATED FILTERS + LOCAL BINDING SCOPES');
$results = $index->search('shoes')
    ->filter('price <= :value', ['value' => 120.0])
    ->filter('rating >= :value', ['value' => 4])
    ->filter('in_stock = true')
    ->sort('price', 'asc')
    ->limit(20)
    ->execute();
assertIds($results, ['shoe-1', 'shoe-5']);

heading('8. FILTER GRAMMAR: OR / NOT / IN / PARENTHESES');
$results = $index->search('shoes')
    ->filter(
        '(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)',
        ['brands' => ['Acme', 'TrailCo'], 'rating' => 3],
    )
    ->sort('price', 'asc')
    ->limit(20)
    ->execute();
assertIds($results, ['shoe-3', 'shoe-1', 'shoe-5', 'shoe-2']);

heading('9. SORT + FIELD PROJECTION + HIT ACCESS');
$results = $index->search('shoes')
    ->sort('price', 'desc')
    ->fields('title', 'brand', 'price')
    ->limit(2)
    ->execute();
assertIds($results, ['shoe-4', 'shoe-2']);
same('Acme Racing Shoes', $results->hits[0]->get('title'), 'field mapping access');
same('Acme', $results->hits[0]->get('brand'), 'projected brand');
same(159.0, $results->hits[0]->get('price'), 'projected price');
check(!array_key_exists('rating', $results->hits[0]->fields), 'projection excludes rating');

heading('10. PAGINATION');
$allSorted = $index->search('shoes')->sort('price', 'asc')->limit(20)->execute();
$window = $index->search('shoes')->sort('price', 'asc')->offset(1)->limit(2)->execute();
$page = $index->search('shoes')->sort('price', 'asc')->page(2, 2)->execute();
same(array_slice(ids($allSorted), 1, 2), ids($window), 'offset/limit window');
same(array_slice(ids($allSorted), 2, 2), ids($page), 'one-based page');
same($allSorted->totalHits, $window->totalHits, 'window total hits');
same($allSorted->totalHits, $page->totalHits, 'page total hits');

heading('11. BUILDER MISUSE');
expectInvalid(static fn () => $index->search('shoes')->page(2, 20)->offset(20), 'page + offset rejected');
expectInvalid(static fn () => $index->search('shoes')->offset(20)->page(2, 20), 'offset + page rejected');
expectInvalid(static fn () => $index->search('shoes')->page(2, 20)->limit(20), 'page + limit rejected');
expectInvalid(static fn () => $index->search('shoes')->limit(-1), 'negative limit rejected');
expectInvalid(static fn () => $index->search('shoes')->offset(-1), 'negative offset rejected');

heading('12. FACETS');
$results = $index->search('shoes')
    ->facet('brand', 'brands', 10, 'count_desc')
    ->facet('category', null, 10, 'count_desc')
    ->limit(20)
    ->execute();
$brandCounts = [];
foreach ($results->facets['brands'] as $bucket) $brandCounts[(string) $bucket['value']] = $bucket['count'];
same(['Acme' => 2, 'TrailCo' => 2, 'Budget' => 1], $brandCounts, 'brand facet');
$categoryCounts = [];
foreach ($results->facets['category'] as $bucket) $categoryCounts[(string) $bucket['value']] = $bucket['count'];
same(['shoes' => 5], $categoryCounts, 'category facet');

heading('13. FILTERED FACET');
$results = $index->search('shoes')
    ->filter('in_stock = true')
    ->facet('brand', null, 10, 'count_desc')
    ->limit(20)
    ->execute();
$brandCounts = [];
foreach ($results->facets['brand'] as $bucket) $brandCounts[(string) $bucket['value']] = $bucket['count'];
same(['TrailCo' => 2, 'Acme' => 1, 'Budget' => 1], $brandCounts, 'filtered facet');

heading('14. ROOT AGGREGATIONS');
$results = $index->browse()
    ->filter("category = 'shoes'")
    ->aggregate(Agg::count()->as('products'))
    ->aggregate(Agg::sum('price')->as('sum_price'))
    ->aggregate(Agg::avg('price')->as('avg_price'))
    ->aggregate(Agg::min('price')->as('min_price'))
    ->aggregate(Agg::max('price')->as('max_price'))
    ->aggregate(Agg::stddev('price')->as('stddev_price'))
    ->limit(0)
    ->execute();
same(5, $results->aggregations['products'], 'root count');
closeEnough(535.0, (float) $results->aggregations['sum_price'], 'root sum');
closeEnough(107.0, (float) $results->aggregations['avg_price'], 'root avg');
closeEnough(49.0, (float) $results->aggregations['min_price'], 'root min');
closeEnough(159.0, (float) $results->aggregations['max_price'], 'root max');
check(is_finite((float) $results->aggregations['stddev_price']) && $results->aggregations['stddev_price'] > 0, 'root stddev');

heading('15. GROUP BY BRAND + COUNT / AVG + ORDER');
$results = $index->search('shoes')
    ->group(
        Group::by('brand')
            ->limit(10)
            ->precision(100)
            ->orderBy(
                GroupingOrder::desc(Agg::count()),
                GroupingOrder::asc(G::field('brand')),
            )
            ->output(
                Agg::count()->as('products'),
                Agg::avg('price')->as('avg_price'),
            )
    )
    ->execute();
$byBrand = groupsByValue(groupingGroups($results));
same(2, $byBrand['Acme']->fields['products'], 'Acme group count');
closeEnough(124.0, (float) $byBrand['Acme']->fields['avg_price'], 'Acme avg');
same(2, $byBrand['TrailCo']->fields['products'], 'TrailCo group count');
closeEnough(119.0, (float) $byBrand['TrailCo']->fields['avg_price'], 'TrailCo avg');
same(['Acme', 'TrailCo', 'Budget'], array_map(static fn ($g): string => (string) $g->value, groupingGroups($results)), 'group ordering');

heading('16. NESTED GROUPING: CATEGORY -> BRAND');
$results = $index->browse()
    ->filter('category IN (:categories)', ['categories' => ['shoes', 'bags']])
    ->group(
        Group::by('category')
            ->limit(10)
            ->orderBy(GroupingOrder::desc(Agg::count()))
            ->output(Agg::count()->as('products'))
            ->subgroup(
                Group::by('brand')
                    ->limit(10)
                    ->orderBy(
                        GroupingOrder::desc(Agg::count()),
                        GroupingOrder::asc(G::field('brand')),
                    )
                    ->output(Agg::count()->as('products'), Agg::avg('price')->as('avg_price'))
            )
    )
    ->execute();
$categories = groupsByValue(groupingGroups($results));
same(5, $categories['shoes']->fields['products'], 'nested shoes count');
same(1, $categories['bags']->fields['products'], 'nested bags count');
$shoeBrands = groupsByValue($categories['shoes']->groups);
same(2, $shoeBrands['Acme']->fields['products'], 'nested Acme count');

heading('17. GROUP ALIAS + OUTPUT');
$results = $index->search('shoes')
    ->group(
        Group::by('brand')
            ->limit(10)
            ->alias('revenue', Agg::sum('price'))
            ->orderBy(GroupingOrder::desc(G::aliasRef('revenue')))
            ->output(Agg::count()->as('products'), G::aliasRef('revenue')->as('revenue'))
    )
    ->execute();
$byBrand = groupsByValue(groupingGroups($results));
closeEnough(248.0, (float) $byBrand['Acme']->fields['revenue'], 'Acme revenue');
closeEnough(238.0, (float) $byBrand['TrailCo']->fields['revenue'], 'TrailCo revenue');

heading('18. GROUP KEEP FILTER');
$results = $index->search('shoes')
    ->group(
        Group::by('brand')
            ->limit(10)
            ->keep(Gf::regex(G::field('brand'), '^A.*'))
            ->output(Agg::count()->as('products'))
    )
    ->execute();
same(['Acme'], array_map(static fn ($g): string => (string) $g->value, groupingGroups($results)), 'group keep regex');

heading('19. SUMMARY AGGREGATION -> GROUP HIT CHILDREN');
$results = $index->search('shoes')
    ->group(
        Group::by('brand')
            ->output(Agg::count()->as('products'), Agg::summary()->as('hits'))
    )
    ->execute();
$groupedHitIds = [];
foreach (groupingGroups($results) as $group) {
    check($group->hits !== [], 'summary aggregation keeps hit children under group');
    foreach ($group->hits as $hit) $groupedHitIds[] = $hit->id;
}
sort($groupedHitIds);
$expected = ['shoe-1', 'shoe-2', 'shoe-3', 'shoe-4', 'shoe-5'];
sort($expected);
same($expected, $groupedHitIds, 'summary aggregation grouped hits');

heading('20. GROUP CONTINUATION TOKEN PRESERVATION [DEFERRED]');
echo "Native grouping ABI does not yet return continuation tokens; parity matches Python's documented deferral.\n";

heading('21. TYPED QUERY DSL');
$results = $index->query(Query::contains('title', 'shoes'))
    ->ranking('default')
    ->limit(20)
    ->execute();
$actual = ids($results);
sort($actual);
$expected = ['shoe-1', 'shoe-2', 'shoe-3', 'shoe-4', 'shoe-5'];
sort($expected);
same($expected, $actual, 'typed contains query');

$results = $index->query(Query::contains('title', 'shoes'))
    ->input('category_boost', 1.5)
    ->rankProperty('showcase', 'true')
    ->ranking('default')
    ->limit(5)
    ->execute();
same(5, $results->totalHits, 'query input and rank-property namespaces');

heading('21A. TYPED QUERY FAMILY PARITY');
$phrase = $index->query(Query::contains('title', Query::phrase('quick', 'brown')))->limit(20)->execute();
assertIds($phrase, ['typed-d1']);
$nearIds = ids($index->query(Query::contains('title', Query::near('quick', 'brown')))->limit(20)->execute());
sort($nearIds);
same(['typed-d1', 'typed-d2', 'typed-d6'], $nearIds, 'typed near accepts reverse order and one-gap proximity');
$orderedNearIds = ids($index->query(Query::contains('title', Query::onear('quick', 'brown')))->limit(20)->execute());
sort($orderedNearIds);
same(['typed-d1', 'typed-d6'], $orderedNearIds, 'typed ordered-near rejects reverse order');
$fuzzyIds = ids($index->query(Query::contains('category', Query::fuzzy('foxx')))->limit(20)->execute());
sort($fuzzyIds);
same(['typed-d1', 'typed-d2', 'typed-d3', 'typed-d4', 'typed-d6'], $fuzzyIds, 'typed fuzzy excludes unrelated terms');
$weakAndIds = ids($index->query(Query::weakAnd(Query::contains('title', 'quick'), Query::contains('title', 'fox')))->limit(20)->execute());
sort($weakAndIds);
same(['typed-d1', 'typed-d2', 'typed-d3', 'typed-d4', 'typed-d6'], $weakAndIds, 'typed weak-and keeps one-term-only matches');
$wandIds = ids($index->query(Query::wand('title', ['quick' => 10, 'fox' => 1]))->limit(20)->execute());
sort($wandIds);
same(['typed-d1', 'typed-d2', 'typed-d3', 'typed-d4', 'typed-d6'], $wandIds, 'typed WAND keeps one-term-only matches');
$sameElement = $index->query(Query::contains('people', Query::sameElement(Query::contains('first', 'Ada'), Query::contains('last', 'Lovelace'))))->limit(20)->execute();
assertIds($sameElement, ['people-1']);
$geoLocation = $index->query(Query::geoLocation('location', 37.3, -121.9, '10km'))->limit(10)->execute();
assertIds($geoLocation, ['geo-inside']);
$geoBox = $index->query(Query::geoBox('location', 37.0, -122.2, 37.6, -121.5))->limit(10)->execute();
assertIds($geoBox, ['geo-inside']);

heading('RANKING, SCALAR FEATURES, AND OPAQUE FEATURES');
$ranked = $index->search('shoes')
    ->ranking('boosted')
    ->input('boost', 3.0)
    ->limit(20)
    ->execute();
check($ranked->hits !== [], 'ranked query returns hits');
foreach ($ranked->hits as $hit) {
    check($hit->score() !== null, 'ranked hit has score');
    closeEnough(3.0, (float) $hit->matchFeatures['query(boost)'], 'scalar match feature');
}

$rankPropertyResults = $index->search('shoes')
    ->ranking('boosted')
    ->rankProperty('boost', '4.0')
    ->limit(20)
    ->execute();
check($rankPropertyResults->hits !== [], 'rank-property query returns hits');
foreach ($rankPropertyResults->hits as $hit) {
    closeEnough(4.0, (float) $hit->matchFeatures['query(boost)'], 'rank property feature');
}

heading('TWO-PHASE RANKING / SECOND-PHASE RERANK');
$firstPhase = $index->search('candidate')->ranking('first_phase_only')->limit(4)->execute();
assertIds($firstPhase, ['rerank-a', 'rerank-b', 'rerank-c', 'rerank-d']);
same([100.0, 90.0, 80.0, 70.0], array_map(static fn ($h): float => (float) $h->score(), $firstPhase->hits), 'first-phase scores');
$twoPhase = $index->search('candidate')->ranking('two_phase')->limit(4)->execute();
assertIds($twoPhase, ['rerank-b', 'rerank-c', 'rerank-a', 'rerank-d']);
same(4, $twoPhase->totalHits, 'two-phase total hits');
same([100.0, 90.0, 1.0], array_map(static fn ($h): float => (float) $h->score(), array_slice($twoPhase->hits, 0, 3)), 'second-phase rerank scores');
check(is_finite((float) $twoPhase->hits[3]->score()) && $twoPhase->hits[3]->score() !== 1000.0, 'outside rerank window retains finite score');

heading('TENSOR READBACK + NEAREST NEIGHBOR');
$nearest = $index->query(Query::nearestNeighbor('embedding', 'query_embedding'))
    ->ranking('semantic')
    ->input('query_embedding', embedding(SHOE_1_VECTOR))
    ->limit(3)
    ->execute();
same('shoe-1', $nearest->hits[0]->id, 'nearest neighbor top hit');
check($nearest->hits[0]->score() > $nearest->hits[1]->score(), 'nearest-neighbor score ordering');
check($nearest->coverage !== null, 'nearest-neighbor coverage');
check($nearest->hits[0]->matchFeatures['query(query_embedding)'] instanceof Bytes, 'opaque tensor feature stays binary');

heading('HYBRID TEXT + VECTOR SEARCH');
$lexical = $index->query(Query::contains('title', 'shoes'))->limit(20)->execute();
check(!in_array('bag-1', ids($lexical), true), 'bag is not lexical shoes match');
$hybrid = $index->query(Query::anyOf(
        Query::contains('title', 'shoes'),
        Query::nearestNeighbor('embedding', 'query_embedding'),
    ))
    ->ranking('hybrid')
    ->input('query_embedding', embedding(BAG_1_VECTOR))
    ->limit(20)
    ->execute();
$hybridIds = ids($hybrid);
check(in_array('bag-1', $hybridIds, true), 'vector-only candidate survives OR union');
foreach (['shoe-1', 'shoe-2', 'shoe-3', 'shoe-4', 'shoe-5'] as $id) {
    check(in_array($id, $hybridIds, true), "lexical candidate {$id} survives OR union");
}
same('bag-1', $hybrid->hits[0]->id, 'hybrid top hit');
$top = $hybrid->hits[0];
closeEnough(0.0, (float) $top->matchFeatures['bm25(title)'], 'vector-only bm25 feature');
check($top->matchFeatures['closeness(embedding)'] > 0.99, 'vector closeness');
closeEnough(
    (float) $top->score(),
    (float) $top->matchFeatures['bm25(title)'] + 10.0 * (float) $top->matchFeatures['closeness(embedding)'],
    'hybrid score consistency',
);
$lexicalHit = current(array_values(array_filter($hybrid->hits, static fn ($hit): bool => $hit->id === 'shoe-1')));
check($lexicalHit !== false, 'lexical candidate remains in hybrid hits');
check($lexicalHit->matchFeatures['bm25(title)'] > 0, 'lexical BM25 contribution');
closeEnough(
    (float) $lexicalHit->score(),
    (float) $lexicalHit->matchFeatures['bm25(title)'] + 10.0 * (float) $lexicalHit->matchFeatures['closeness(embedding)'],
    'lexical hybrid score consistency',
);

heading('COVERAGE AND DEGRADATION');
$normal = $index->search('shoes')->limit(20)->execute();
check($normal->coverage !== null && $normal->coverage->full === true, 'normal coverage full');
$degraded = $index->browse()
    ->filter('inventory = :value', ['value' => 18])
    ->sort('inventory', 'asc')
    ->rankProperty('vespa.matchphase.degradation.maxfiltercoverage', '0.9')
    ->rankProperty('vespa.matchphase.degradation.attribute', 'inventory')
    ->rankProperty('vespa.matchphase.degradation.maxhits', '10')
    ->limit(10)
    ->execute();
check($degraded->hits !== [], 'degradation query returns hits');
check($degraded->coverage !== null && $degraded->coverage->full === false, 'degraded coverage not full');
check(
    $degraded->coverage->degraded !== null
    && ($degraded->coverage->degraded->timeout === true || $degraded->coverage->degraded->matchPhase === true),
    'degradation reason retained',
);

heading('22. YQL + PROPERTY + PAGINATION');
$results = $index->yql(
        'select * from sources * where title contains text(@q) order by price asc'
    )
    ->property('q', 'shoes')
    ->ranking('default')
    ->page(1, 3)
    ->execute();
assertIds($results, ['shoe-3', 'shoe-1', 'shoe-5']);
same(5, $results->totalHits, 'YQL total hits');

heading('23. YQL + PROGRAMMATIC GROUPING');
$results = $index->yql('select * from sources * where title contains text(@q)')
    ->property('q', 'shoes')
    ->group(Group::by('brand')->limit(10)->output(Agg::count()->as('products')))
    ->execute();
$byBrand = groupsByValue(groupingGroups($results));
same(2, $byBrand['Acme']->fields['products'], 'YQL + grouping Acme');
same(2, $byBrand['TrailCo']->fields['products'], 'YQL + grouping TrailCo');
same(1, $byBrand['Budget']->fields['products'], 'YQL + grouping Budget');

heading('24. STRUCTURED FILTER ERROR');
try {
    $index->search('shoes')->filter('prcie <= :max', ['max' => 100])->execute();
    throw new RuntimeException('unknown field filter should fail');
} catch (SearchKernelException $error) {
    check(in_array('unknown_field', detailCodes($error), true), 'unknown field detail code');
    same('filters.textual[0].source', $error->details[0]['path'], 'unknown field detail path');
}

try {
    $index->search('shoes')->filter('price <= :max', ['unused' => 100])->execute();
    throw new RuntimeException('missing filter binding should fail');
} catch (SearchKernelException $error) {
    check(in_array('unknown_parameter', detailCodes($error), true), 'missing binding detail code');
}

try {
    $index->search('shoes')->filter('price <= :max', ['max' => 100, 'unused' => 200])->execute();
    throw new RuntimeException('unused filter binding should fail');
} catch (SearchKernelException $error) {
    check(in_array('unused_parameter', detailCodes($error), true), 'unused binding detail code');
}

heading('24A. TEXTUAL FILTER SYNTAX ERROR');
try {
    $index->search('shoes')->filter('price <= :max AND (')->execute();
    throw new RuntimeException('malformed textual filter should fail');
} catch (SearchKernelException $error) {
    check(in_array('syntax_error', detailCodes($error), true), 'syntax detail code');
    $syntaxDetails = array_values(array_filter($error->details, static fn (array $detail): bool => $detail['code'] === 'syntax_error'));
    same('filters.textual[0].source', $syntaxDetails[0]['path'], 'syntax detail path');
}

heading('25. PATCH BUILDER');
$index->patch('shoe-1')
    ->set('price', 79.0)
    ->set('rating', 4)
    ->unset('sale_price')
    ->execute();
$updated = $index->get('shoe-1');
same(79.0, $updated['price'], 'patch price');
same(4, $updated['rating'], 'patch rating');
check(!array_key_exists('sale_price', $updated), 'patch unset');
assertIds(
    $index->search('shoes')->filter('price < :max', ['max' => 80])->sort('price', 'asc')->limit(20)->execute(),
    ['shoe-3', 'shoe-1'],
);

heading('26. ADVANCED UPDATE BUILDER');
$index->update('shoe-1')->unset('sale_price')->execute();
$index->update('shoe-1')->set('title', 'Fast Marathon Shoes')->execute();
same('Fast Marathon Shoes', $index->get('shoe-1')['title'], 'advanced string assignment');
check(in_array('shoe-1', ids($index->search('marathon')->execute()), true), 'new title indexed');
check(!in_array('shoe-1', ids($index->search('jumped')->execute()), true), 'old title postings removed');

$index->update('shoe-1')->set('inventory', 20)->execute();
$index->update('shoe-1')->increment('inventory', 5)->execute();
same(25, $index->get('shoe-1')['inventory'], 'increment');
$index->update('shoe-1')->decrement('inventory', 3)->execute();
same(22, $index->get('shoe-1')['inventory'], 'decrement');
$index->update('shoe-1')->multiply('inventory', 2)->execute();
same(44, $index->get('shoe-1')['inventory'], 'multiply');
$index->update('shoe-1')->divide('inventory', 4)->execute();
same(11, $index->get('shoe-1')['inventory'], 'divide');
$index->update('shoe-created')->set('inventory', 7)->create()->execute();
same(7, $index->get('shoe-created')['inventory'], 'create update');

try {
    $index->update('shoe-1')->set('inventory', 16)->condition('inventory == 15')->execute();
    throw new RuntimeException('embedded update conditions must be Unsupported');
} catch (SearchKernelException $error) {
    same('Unsupported', $error->errorCode, 'conditional update unsupported error');
}

heading('27. DELETE');
check($index->delete('shoe-3'), 'first delete true');
same(null, $index->get('shoe-3'), 'deleted document absent');
$results = $index->search('shoes')->limit(20)->execute();
check(!in_array('shoe-3', ids($results), true), 'deleted search hit absent');
same(4, $results->totalHits, 'total hits after delete');
$results = $index->search('shoes')->facet('brand')->limit(20)->execute();
$brands = array_map(static fn (array $bucket): string => (string) $bucket['value'], $results->facets['brand']);
check(!in_array('Budget', $brands, true), 'facet reflects delete');
check(!$index->delete('shoe-3'), 'duplicate delete false');

heading('28. UPSERT REPLACEMENT');
$upsertSpec = $parityIntents['upsert_replacement'];
$replacement = $upsertSpec['replacement'];
$replacement = normalizeCorpusDocument($replacement);
$vectorDocumentId = $replacement['vector_document_id'];
unset($replacement['vector_document_id']);
$index->upsert(['_id' => $upsertSpec['document_id'], ...$replacement, 'embedding' => embedding(corpusVector($vectorDocumentId))]);
$doc = $index->get($upsertSpec['document_id']);
same(119.0, $doc['price'], 'upsert replacement price');
check(str_contains($doc['title'], 'Updated Edition'), 'upsert replacement title');
$updatedSearch = $index->search($upsertSpec['search_query'])->limit(20)->execute();
same([$upsertSpec['document_id']], ids($updatedSearch), 'replacement searchable');
$parity['upsert_replacement'] = ['price' => $doc['price'], 'updated_title_matches' => str_contains($doc['title'], 'Updated'), 'search_ids' => ids($updatedSearch)];

writeRealEngineParity($parity);

heading('29. INITIAL PHP CLIENT API E2E CHECKS PASSED');
echo "Real-engine PHP facade scenario complete; harness will restart searchkerneld for reopen checks.\n";

/*
Executable capability evidence from this real-engine showcase.

SKC: query.term/basic kinds=ergonomic,e2e runtime=http
SKC: query.phrase/basic kinds=ergonomic,e2e runtime=http
SKC: query.proximity/basic kinds=ergonomic,e2e runtime=http
SKC: query.fuzzy/basic kinds=ergonomic,e2e runtime=http
SKC: query.same_element/basic kinds=ergonomic,e2e runtime=http
SKC: query.geo/basic kinds=ergonomic,e2e runtime=http
SKC: query.weighted_and_wand/basic kinds=ergonomic,e2e runtime=http
SKC: query.boolean/basic kinds=ergonomic,e2e runtime=http
SKC: query.nearest_neighbor/basic kinds=ergonomic,e2e runtime=http
SKC: filter.textual/basic kinds=ergonomic,e2e runtime=http
SKC: search.sorting/basic kinds=ergonomic,e2e runtime=http
SKC: search.search_fields/basic kinds=ergonomic,e2e runtime=http
SKC: search.pagination/basic kinds=ergonomic,e2e runtime=http
SKC: result.docsum_field_projection/basic kinds=ergonomic,e2e runtime=http
SKC: result.named_summary_class/basic kinds=ergonomic,e2e runtime=http
SKC: result.highlighting/basic kinds=ergonomic,e2e runtime=http
SKC: result.tensor_document_readback/basic kinds=ergonomic,e2e runtime=http
SKC: grouping.structured/basic kinds=ergonomic,e2e runtime=http
SKC: grouping.aggregations/basic kinds=ergonomic,e2e runtime=http
SKC: grouping.summary_aggregation/basic kinds=ergonomic,e2e runtime=http
SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=http
SKC: update.arithmetic/basic kinds=ergonomic,e2e runtime=http
SKC: update.create_if_nonexistent/basic kinds=ergonomic,e2e runtime=http
SKC: tensor.dense_document_crud/basic kinds=ergonomic,e2e runtime=http
SKC: query.yql/basic kinds=ergonomic,e2e runtime=http
SKC: query.text_weak_and/basic kinds=ergonomic,e2e runtime=http
SKC: ranking.profile_selection/basic kinds=ergonomic,e2e runtime=http
SKC: ranking.numeric_query_inputs/basic kinds=ergonomic,e2e runtime=http
SKC: ranking.tensor_query_inputs/basic kinds=ergonomic,e2e runtime=http
SKC: ranking.rank_properties/basic kinds=ergonomic,e2e runtime=http
SKC: result.match_features/basic kinds=ergonomic,e2e runtime=http
SKC: result.grouping_tree/basic kinds=ergonomic,e2e runtime=http
SKC: result.degradation_metadata/basic kinds=ergonomic,e2e runtime=http
*/
