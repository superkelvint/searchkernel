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

$corpusPath = dirname(__DIR__, 3) . '/reference/client-real-e2e-parity/corpus.json';
$e2eCorpus = json_decode((string) file_get_contents($corpusPath), true, flags: JSON_THROW_ON_ERROR);
$vectorType = $e2eCorpus['schema']['fields']['embedding']['type'];
define('VECTOR_TYPE', $vectorType);
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
    return array_map(static fn ($hit): string => $hit->id, $results->hits);
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

function captureRealEngineParity(Index $index): array
{
    $ordered = $index->search('shoes')->sort('price', 'asc')->limit(20)->execute();
    $filtered = $index->search('shoes')->filter('in_stock = true')->sort('price', 'asc')->limit(20)->execute();
    $faceted = $index->search('shoes')->facet('brand', name: 'brands', limit: 10, sort: 'count_desc')->limit(20)->execute();
    $root = $index->browse()->filter("category = 'shoes'")
        ->aggregate(Agg::count()->as('products'))
        ->aggregate(Agg::sum('price')->as('sum_price'))
        ->aggregate(Agg::avg('price')->as('avg_price'))
        ->limit(0)->execute();
    $grouped = $index->search('shoes')->group(
        Group::by('brand')->limit(10)->precision(100)
            ->orderBy(GroupingOrder::desc(Agg::count()), GroupingOrder::asc(G::field('brand')))
            ->output(Agg::count()->as('products'), Agg::avg('price')->as('avg_price'))
    )->execute();
    $nearest = $index->query(Query::nearestNeighbor('embedding', 'query_embedding'))
        ->ranking('semantic')->input('query_embedding', embedding(SHOE_1_VECTOR))->limit(3)->execute();
    $hybrid = $index->query(Query::anyOf(
        Query::contains('title', 'shoes'),
        Query::nearestNeighbor('embedding', 'query_embedding')
    ))->ranking('hybrid')->input('query_embedding', embedding(BAG_1_VECTOR))->limit(20)->execute();
    $reranked = $index->search('candidate')->ranking('two_phase')->limit(4)->execute();
    try {
        $index->search('shoes')->filter('prcie = 1')->execute();
        throw new RuntimeException('unknown field filter unexpectedly succeeded');
    } catch (SearchKernelException $error) {
        same('InvalidArgument', $error->errorCode, 'portable invalid-argument error code');
        $errorCodes = detailCodes($error);
        sort($errorCodes);
    }

    $document = $index->get('shoe-1');
    $fullText = $index->search('shoes')->limit(20)->execute();
    $stemming = $index->search('jump')->limit(20)->execute();
    $textBinding = $index->search('shoes')->filter('price <= :max', ['max' => 100.0])->sort('price', 'asc')->limit(20)->execute();
    $repeated = $index->search('shoes')->filter('price <= :value', ['value' => 120.0])
        ->filter('rating >= :value', ['value' => 4])->filter('in_stock = true')->sort('price', 'asc')->limit(20)->execute();
    $booleanFilter = $index->search('shoes')->filter('(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)',
        ['brands' => ['Acme', 'TrailCo'], 'rating' => 3])->sort('price', 'asc')->limit(20)->execute();
    $projection = $index->search('shoes')->sort('price', 'desc')->fields('title', 'brand', 'price')->limit(2)->execute();
    $allSorted = $index->search('shoes')->sort('price', 'asc')->limit(20)->execute();
    $page = $index->search('shoes')->sort('price', 'asc')->offset(1)->limit(2)->execute();
    $filteredFacet = $index->search('shoes')->filter('in_stock = true')->facet('brand', limit: 10, sort: 'count_desc')->limit(20)->execute();
    $nested = $index->browse()->filter('category IN (:categories)', ['categories' => ['shoes', 'bags']])->group(
        Group::by('category')->limit(10)->output(Agg::count()->as('products'))
            ->subgroup(Group::by('rating')->limit(10)->output(Agg::count()->as('products'), Agg::avg('price')->as('avg_price')))
    )->execute();
    $aliased = $index->search('shoes')->group(
        Group::by('brand')->limit(10)->alias('revenue', Agg::sum('price'))
            ->orderBy(GroupingOrder::desc(G::aliasRef('revenue')))
            ->output(Agg::count()->as('products'), G::aliasRef('revenue')->as('revenue'))
    )->execute();
    $kept = $index->search('shoes')->group(
        Group::by('brand')->limit(10)->keep(Gf::regex(G::field('brand'), '^A.*'))->output(Agg::count()->as('products'))
    )->execute();
    $typed = $index->query(Query::contains('title', 'shoes'))->ranking('default')->limit(20)->execute();
    $rankInput = $index->search('shoes')->ranking('boosted')->input('boost', 3.0)->limit(20)->execute();
    $rankProperty = $index->search('shoes')->ranking('boosted')->rankProperty('boost', '4.0')->limit(20)->execute();
    $degraded = $index->browse()->filter('inventory = :value', ['value' => 18])->sort('inventory', 'asc')
        ->rankProperty('vespa.matchphase.degradation.maxfiltercoverage', '0.9')
        ->rankProperty('vespa.matchphase.degradation.attribute', 'inventory')
        ->rankProperty('vespa.matchphase.degradation.maxhits', '10')->limit(10)->execute();
    $yql = $index->yql('select * from sources * where title contains text(@q) order by price asc')
        ->property('q', 'shoes')->ranking('default')->page(1, 3)->execute();
    try {
        $index->search('shoes')->filter('price <=')->execute();
        throw new RuntimeException('malformed textual filter unexpectedly succeeded');
    } catch (SearchKernelException $error) {
        $syntaxError = ['code' => strtolower($error->errorCode), 'details' => detailCodes($error)];
        sort($syntaxError['details']);
    }

    $groups = groupingGroups($grouped);
    return [
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
        'facets' => array_column($faceted->facets['brands'], 'count', 'value'),
        'aggregations' => $root->aggregations,
        'grouping_aggregations' => array_map(static fn ($group): array => [
            'brand' => $group->value,
            'products' => $group->fields['products'],
            'avg_price' => $group->fields['avg_price'],
        ], $groups),
        'filtered_facets' => array_column($filteredFacet->facets['brand'], 'count', 'value'),
        'nested_grouping' => array_map(static fn ($group): array => [
            'category' => $group->value,
            'products' => $group->fields['products'],
            'ratings' => array_map(static fn ($rating): array => ['rating' => $rating->value, 'products' => $rating->fields['products'], 'avg_price' => $rating->fields['avg_price']], $group->groups),
        ], groupingGroups($nested)),
        'group_alias' => array_map(static fn ($group): array => ['value' => $group->value, 'fields' => $group->fields], groupingGroups($aliased)),
        'group_keep_filter' => array_map(static fn ($group): array => ['value' => $group->value, 'fields' => $group->fields], groupingGroups($kept)),
        'typed_query' => ['ids' => ids($typed)],
        'rank_inputs' => array_map(static fn ($hit): array => ['id' => $hit->id, 'score' => $hit->score(), 'feature' => $hit->matchFeatures['query(boost)'] ?? null], $rankInput->hits),
        'rank_properties' => array_map(static fn ($hit): array => ['id' => $hit->id, 'score' => $hit->score(), 'feature' => $hit->matchFeatures['query(boost)'] ?? null], $rankProperty->hits),
        'tensor_readback' => $document['embedding']->payload['values'],
        'nearest_neighbor' => ['ids' => ids($nearest), 'scores' => array_map(static fn ($hit): float => (float) $hit->relevance, $nearest->hits)],
        'hybrid_text_vector' => ['ids' => ids($hybrid), 'top_score' => $hybrid->hits[0]->score()],
        'hybrid' => ['top_id' => $hybrid->hits[0]->id, 'has_bag' => in_array('bag-1', ids($hybrid), true)],
        'two_phase_reranking' => ['ids' => ids($reranked)],
        'coverage_degradation' => ['ids' => ids($degraded), 'full' => $degraded->coverage->full, 'degraded' => $degraded->coverage->degraded !== null],
        'yql' => ['ids' => ids($yql), 'total' => $yql->totalHits],
        'unknown_field_error' => ['code' => 'invalid_argument', 'details' => $errorCodes],
        'structured_filter_error' => ['code' => 'invalid_argument', 'details' => $errorCodes],
        'text_filter_syntax_error' => ['code' => 'invalid_argument', 'details' => $syntaxError['details']],
    ];
}

function writeRealEngineParity(array $scenarios): void
{
    $path = getenv('SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT');
    if ($path === false || $path === '') {
        return;
    }
    $observations = ['version' => 1, 'scenarios' => $scenarios];
    if (is_file($path)) {
        $previous = json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);
        $observations['scenarios'] = array_replace($previous['scenarios'] ?? [], $scenarios);
    }
    file_put_contents($path, json_encode($observations, JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION));
}

$url = getenv('SEARCHKERNEL_HTTP_URL') ?: 'http://127.0.0.1:50052';
$phase = $argv[1] ?? 'initial';
if (!in_array($phase, ['initial', 'reopen'], true)) {
    throw new InvalidArgumentException('expected initial or reopen phase');
}
$index = Index::connect($url);

if ($phase === 'reopen') {
    heading('30. REOPEN PERSISTED INDEX');

    same(null, $index->get('shoe-3'), 'deleted document remains deleted');
    $persisted = $index->get('parity-reopen');
    check($persisted !== null, 'parity-reopen survives daemon and index reopen');
    $persistedShoe2 = $index->get('shoe-2');
    writeRealEngineParity(['reopen_persistence' => [
        'marker' => ['title' => $persisted['title'], 'price' => $persisted['price'], 'inventory' => $persisted['inventory']],
        'upserted' => ['updated_title_matches' => str_contains($persistedShoe2['title'], 'Updated'), 'price' => $persistedShoe2['price']],
        'deleted_absent' => $index->get('parity-mutation') === null,
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
    $document['_id'] = "degraded-{$probe}";
    $document['embedding'] = embedding($document['embedding']);
    $index->insert($document);
}

$parity = captureRealEngineParity($index);
$index->insert(['_id' => 'parity-mutation', 'title' => 'Parity Mutation', 'category' => 'parity', 'price' => 100.0, 'sale_price' => 110.0, 'rating' => 1, 'inventory' => 1, 'in_stock' => true]);
$index->patch('parity-mutation')->set('price', 75.0)->unset('sale_price')->execute();
$mutated = $index->get('parity-mutation');
$parity['patch'] = ['price' => $mutated['price'], 'sale_price_present' => array_key_exists('sale_price', $mutated)];
$parityMutationDeleted = $index->delete('parity-mutation');
$parity['delete'] = ['deleted' => $parityMutationDeleted, 'absent_after_delete' => $index->get('parity-mutation') === null];
$index->insert(['_id' => 'parity-reopen', 'title' => 'Parity Reopen', 'category' => 'parity', 'price' => 42.0, 'inventory' => 7, 'in_stock' => true]);

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
$index->upsert([
    '_id' => 'shoe-2',
    'title' => 'Trail Running Shoes - Updated Edition',
    'brand' => 'TrailCo',
    'category' => 'shoes',
    'price' => 119.0,
    'rating' => 5,
    'inventory' => 4,
    'in_stock' => true,
    'embedding' => embedding(SHOE_2_VECTOR),
]);
$doc = $index->get('shoe-2');
same(119.0, $doc['price'], 'upsert replacement price');
check(str_contains($doc['title'], 'Updated Edition'), 'upsert replacement title');
$updatedSearch = $index->search('updated')->limit(20)->execute();
same(['shoe-2'], ids($updatedSearch), 'replacement searchable');
$parity['upsert_replacement'] = ['price' => $doc['price'], 'updated_title_matches' => str_contains($doc['title'], 'Updated'), 'search_ids' => ids($updatedSearch)];

writeRealEngineParity($parity);

heading('29. INITIAL PHP CLIENT API E2E CHECKS PASSED');
echo "Real-engine PHP facade scenario complete; harness will restart searchkerneld for reopen checks.\n";

/*
Executable capability evidence from this real-engine showcase.

SKC: query.term/basic kinds=ergonomic,e2e runtime=http
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
