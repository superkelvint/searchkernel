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

use SearchKernel\Bytes;
use SearchKernel\Coverage;
use SearchKernel\HttpTransport;
use SearchKernel\Index;
use SearchKernel\Protocol;
use SearchKernel\SearchKernelException;
use SearchKernel\SearchResult;
use SearchKernel\Tensor;
use SearchKernel\Transport;
use SearchKernel\TransportException;
use SearchKernel\ProtocolException;
use SearchKernel\UInt64;
use SearchKernel\ValueObject;

function same(mixed $expected, mixed $actual, string $message): void
{
    if ($expected !== $actual) {
        throw new RuntimeException(
            $message . "\nexpected: " . var_export($expected, true) . "\nactual: " . var_export($actual, true)
        );
    }
}

function truth(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

/** @param class-string<Throwable> $class */
function throws(string $class, callable $operation, string $message): void
{
    try {
        $operation();
    } catch (Throwable $error) {
        if ($error instanceof $class) {
            return;
        }
        throw new RuntimeException($message . ': unexpected ' . $error::class, 0, $error);
    }
    throw new RuntimeException($message . ': no exception');
}

final class RecordingTransport implements Transport
{
    /** @var list<array<string,mixed>> */
    public array $requests = [];

    /** @param array<string,mixed> $request
     *  @return array<string,mixed>
     */
    public function execute(array $request): array
    {
        $this->requests[] = $request;
        $requestType = (string) ($request['payload_type'] ?? '');
        $payload = is_array($request['payload'] ?? null) ? $request['payload'] : [];

        $result = match ($requestType) {
            'SearchKernel_V2_InsertRequest' => [
                'payload_type' => 'SearchKernel_V2_InsertResult',
                'payload' => ['id' => $payload['document']['id'] ?? ''],
            ],
            'SearchKernel_V2_UpsertRequest' => [
                'payload_type' => 'SearchKernel_V2_UpsertResult',
                'payload' => ['id' => $payload['document']['id'] ?? ''],
            ],
            'SearchKernel_V2_GetRequest' => [
                'payload_type' => 'SearchKernel_V2_GetResult',
                'payload' => ['document' => null],
            ],
            'SearchKernel_V2_DeleteRequest' => [
                'payload_type' => 'SearchKernel_V2_DeleteResult',
                'payload' => ['id' => $payload['id'] ?? '', 'deleted' => true],
            ],
            'SearchKernel_V2_PatchRequest' => [
                'payload_type' => 'SearchKernel_V2_PatchResult',
                'payload' => ['id' => $payload['id'] ?? ''],
            ],
            'SearchKernel_V2_UpdateRequest' => [
                'payload_type' => 'SearchKernel_V2_UpdateResult',
                'payload' => ['id' => $payload['id'] ?? ''],
            ],
            'SearchKernel_V2_SearchRequest' => [
                'payload_type' => 'SearchKernel_V2_SearchResult',
                'payload' => [
                    'root' => null,
                    'total_hits' => ['value' => 0, 'relation' => 'Exact'],
                ],
            ],
            'SearchKernel_V2_DescribeRequest' => [
                'payload_type' => 'SearchKernel_V2_DescribeResult',
                'payload' => ['index' => ['name' => 'test']],
            ],
            default => throw new RuntimeException('unexpected test request type: ' . $requestType),
        };

        return [
            'protocol_version' => 2,
            ...$result,
        ];
    }
}

// --------------------------------------------------------------------------
// Portable value boundary.
// --------------------------------------------------------------------------

$document = Protocol::document([
    '_id' => 'doc-1',
    'title' => 'The quick brown fox',
    'count' => PHP_INT_MAX,
    'price' => 12.5,
    'active' => true,
    'tags' => ['one', 'two'],
    'meta' => ['z' => 2, 'a' => 1],
]);
same('doc-1', $document['id'], 'document id');
same(
    ['active', 'count', 'meta', 'price', 'tags', 'title'],
    array_column($document['fields'], 'key'),
    'document fields are canonical key order',
);
same('Int64Value', $document['fields'][1]['value']['kind_type'], 'integers stay int64');
same('Float64Value', $document['fields'][3]['value']['kind_type'], 'floats stay float64');
same('ValueList', $document['fields'][4]['value']['kind_type'], 'lists stay lists');

$decoded = Protocol::decodeDocument($document);
same('doc-1', $decoded['_id'], 'decode document id');
same(PHP_INT_MAX, $decoded['count'], 'decode int');
same(['one', 'two'], $decoded['tags'], 'decode list');
same(['a' => 1, 'z' => 2], $decoded['meta'], 'decode object');

$bytes = new Bytes("\x00\xffA");
$encodedBytes = Protocol::value($bytes);
same('BytesValue', $encodedBytes['kind_type'], 'bytes discriminator');
same([0, 255, 65], $encodedBytes['kind']['value'], 'bytes preserve octets');
$decodedBytes = Protocol::decodeValue($encodedBytes);
truth($decodedBytes instanceof Bytes, 'bytes decode to Bytes');
same($bytes->value, $decodedBytes->value, 'bytes round trip');

$tensor = Tensor::dense('tensor<float>(x[2])', [1, 2.5]);
$encodedTensor = Protocol::value($tensor);
same('TensorValue', $encodedTensor['kind_type'], 'tensor discriminator');
same('DenseTensorValues', $encodedTensor['kind']['payload_type'], 'dense tensor discriminator');
$decodedTensor = Protocol::decodeValue($encodedTensor);
truth($decodedTensor instanceof Tensor, 'tensor decode to Tensor');
same('tensor<float>(x[2])', $decodedTensor->typeSpec, 'tensor type spec round trip');
same([1.0, 2.5], $decodedTensor->payload['values'], 'tensor cells round trip');

throws(
    \InvalidArgumentException::class,
    static fn () => Tensor::dense('tensor<float>(x[1])', [INF]),
    'tensor non-finite cells fail closed',
);
throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::decodeValue(['kind_type' => 'FutureValue', 'kind' => []]),
    'unknown Value unions fail closed',
);

same(
    INF,
    Protocol::decodeValue([
        'kind_type' => 'NonFiniteFloatValue',
        'kind' => ['kind' => 'PositiveInfinity'],
    ]),
    'non-finite result values decode explicitly',
);

// --------------------------------------------------------------------------
// Document and update facade.
// --------------------------------------------------------------------------

$transport = new RecordingTransport();
$index = new Index($transport);

same('doc-1', $index->upsert(['_id' => 'doc-1', 'title' => 'fox'], 'version == 7'), 'upsert id');
$upsert = $transport->requests[array_key_last($transport->requests)];
same('version == 7', $upsert['payload']['condition'], 'upsert condition preserved');

same(null, $index->get('doc-1'), 'missing get returns null');
$getDefault = $transport->requests[array_key_last($transport->requests)];
truth(!array_key_exists('projection', $getDefault['payload']), 'absent get projection stays absent');

same(null, $index->get('doc-1', []), 'missing projected get returns null');
$getEmpty = $transport->requests[array_key_last($transport->requests)];
same([], $getEmpty['payload']['projection']['fields'], 'present-empty get projection is preserved');

truth($index->delete('doc-1', 'version == 7'), 'conditional delete result');
$delete = $transport->requests[array_key_last($transport->requests)];
same('version == 7', $delete['payload']['condition'], 'delete condition preserved');

same(
    'doc-1',
    $index->patch('doc-1')->set('price', 12.5)->unset('sale_price')->execute(),
    'patch result id',
);
$patch = $transport->requests[array_key_last($transport->requests)];
same('SearchKernel_V2_PatchRequest', $patch['payload_type'], 'patch envelope discriminator');
same('SetField', $patch['payload']['operations'][0]['operation_type'], 'patch set discriminator');
same('UnsetField', $patch['payload']['operations'][1]['operation_type'], 'patch unset discriminator');

same(
    'doc-1',
    $index->update('doc-1')
        ->assign('price', 20.0)
        ->clear('sale_price')
        ->add('tags', 'featured')
        ->remove('tags', 'clearance')
        ->increment('inventory', 2)
        ->condition('version == 7')
        ->create(false)
        ->execute(),
    'update result id',
);
$update = $transport->requests[array_key_last($transport->requests)];
same('AssignUpdate', $update['payload']['operations'][0]['operation_type'], 'assign update discriminator');
same('ClearUpdate', $update['payload']['operations'][1]['operation_type'], 'clear update discriminator');
same('AddUpdate', $update['payload']['operations'][2]['operation_type'], 'add update discriminator');
same('RemoveUpdate', $update['payload']['operations'][3]['operation_type'], 'remove update discriminator');
same('ArithmeticUpdate', $update['payload']['operations'][4]['operation_type'], 'arithmetic discriminator');
same('Increment', $update['payload']['operations'][4]['operation']['operator'], 'arithmetic operator');
same(['value' => false], $update['payload']['create'], 'update create presence preserved');
same('version == 7', $update['payload']['condition'], 'update condition preserved');

// --------------------------------------------------------------------------
// Search request facade and canonical namespaces.
// --------------------------------------------------------------------------

$request = $index
    ->search('brown fox')
    ->filter('price <= :value', ['value' => 120])
    ->filter('rating >= :value', ['value' => 4])
    ->sort('price', 'desc', 'last')
    ->sortRelevance()
    ->fields()
    ->summary('default')
    ->highlight('title')
    ->ranking('bm25')
    ->rankProperty('foo', 123)
    ->input('embedding', Tensor::dense('tensor<float>(x[2])', [1, 2]))
    ->property('trace.level', 2)
    ->page(2, 20)
    ->timeout(1000)
    ->request();

same('SearchKernelTextQuery', $request['query']['source_type'], 'search query source');
same(2, count($request['filters']['textual']), 'repeated filters remain separate clauses');
same('value', $request['filters']['textual'][0]['bindings'][0]['key'], 'first clause binding key');
same(120, $request['filters']['textual'][0]['bindings'][0]['value']['kind']['value'], 'first clause binding value');
same(4, $request['filters']['textual'][1]['bindings'][0]['value']['kind']['value'], 'second clause binding value');
same('FieldSortTarget', $request['sort'][0]['target']['target_type'], 'field sort target');
same('Desc', $request['sort'][0]['direction'], 'field sort direction');
same('MissingLast', $request['sort'][0]['missing']['policy_type'], 'missing policy');
same('RelevanceSortTarget', $request['sort'][1]['target']['target_type'], 'relevance sort target');
same([], $request['presentation']['fields']['fields'], 'present-empty projection survives search builder');
same(['title'], $request['presentation']['highlighting']['fields'], 'highlight fields');
same('bm25', $request['ranking']['profile'], 'ranking profile');
same([['key' => 'foo', 'value' => '123']], $request['ranking']['properties'], 'rank property namespace');
same('LiteralQueryInput', $request['inputs'][0]['input_type'], 'query input namespace');
same('TensorValue', $request['inputs'][0]['input']['value']['kind_type'], 'tensor query input');
same([['key' => 'trace.level', 'value' => Protocol::value(2)]], $request['properties'], 'generic property namespace');
same(['value' => 20], $request['offset'], 'page offset');
same(['value' => 20], $request['limit'], 'page limit');
same(['value' => 1000], $request['timeout_ms'], 'timeout');

same(
    'MatchAllQuery',
    $index->browse()->request()['query']['source_type'],
    'browse uses explicit match-all query source',
);
$yql = $index->yql('select * from sources * where title contains text(@q)', ['q' => 'fox'])->request();
same('YqlQuery', $yql['query']['source_type'], 'YQL source discriminator');
same('q', $yql['query']['source']['parameters'][0]['key'], 'YQL parameter namespace');

throws(
    \InvalidArgumentException::class,
    static fn () => $index->search(' '),
    'blank search requires browse()',
);
throws(
    \InvalidArgumentException::class,
    static fn () => $index->browse()->page(2, 20)->limit(20),
    'page and limit cannot mix',
);
throws(
    \InvalidArgumentException::class,
    static fn () => $index->browse()->page(PHP_INT_MAX, 2),
    'page arithmetic is checked',
);
throws(
    \InvalidArgumentException::class,
    static fn () => $index->browse()->sort('price', 'sideways'),
    'invalid sort direction rejected',
);

// --------------------------------------------------------------------------
// Recursive result fidelity.
// --------------------------------------------------------------------------

$result = SearchResult::fromPayload([
    'root' => [
        'node_type' => 'ResultGroup',
        'node' => [
            'id' => 'root',
            'label' => 'root',
            'fields' => [
                ['key' => 'count', 'value' => ['kind_type' => 'UInt64Value', 'kind' => ['value' => 2]]],
            ],
            'continuations' => [['key' => 'next', 'value' => 'opaque-token']],
            'children' => [
                [
                    'node_type' => 'Hit',
                    'node' => [
                        'id' => 'doc-1',
                        'relevance' => [
                            'value_type' => 'PositiveInfinityRelevance',
                            'value' => [],
                        ],
                        'fields' => [
                            ['key' => 'title', 'value' => ['kind_type' => 'StringValue', 'kind' => ['value' => 'fox']]],
                            ['key' => 'raw', 'value' => $encodedBytes],
                        ],
                        'match_features' => [
                            ['key' => 'score', 'value' => ['kind_type' => 'Float64Value', 'kind' => ['value' => 1.5]]],
                        ],
                        'sort_values' => [
                            ['kind_type' => 'Int64Value', 'kind' => ['value' => 7]],
                        ],
                        'highlights' => [[
                            'field' => 'title',
                            'fragments' => [[
                                'text' => 'fox',
                                'matches' => [['start_byte' => 0, 'end_byte' => 3]],
                            ]],
                        ]],
                    ],
                ],
                [
                    'node_type' => 'ResultGroup',
                    'node' => [
                        'id' => 'nested',
                        'label' => 'brand',
                        'value' => ['kind_type' => 'StringValue', 'kind' => ['value' => 'Acme']],
                        'children' => [],
                    ],
                ],
            ],
        ],
    ],
    'total_hits' => ['value' => '18446744073709551615', 'relation' => 'LowerBound'],
    'coverage' => [
        'coverage_percent' => ['value' => 99.5],
        'full' => ['value' => false],
        'degraded' => [
            'timeout' => ['value' => true],
            'metadata' => [],
        ],
    ],
    'timing' => [
        'query_time_seconds' => ['value' => 0.01],
        'search_time_seconds' => ['value' => 0.02],
        'metadata' => [],
    ],
    'errors' => [[
        'code' => 17,
        'summary' => 'partial',
        'message' => 'degraded',
        'source' => 'vespa',
        'transient' => ['value' => true],
        'metadata' => [],
    ]],
    'trace' => ['kind_type' => 'StringValue', 'kind' => ['value' => 'trace']],
    'metadata' => [
        ['key' => 'server', 'value' => ['kind_type' => 'StringValue', 'kind' => ['value' => 'local']]],
    ],
]);

same(1, count($result->hits), 'top-level hits preserved');
same(1, count($result->groups), 'top-level groups preserved');
same(INF, $result->hits[0]->relevance, 'infinite relevance preserved');
same(1.5, $result->hits[0]->matchFeatures['score'], 'match features preserved');
same(7, $result->hits[0]->sortValues[0], 'sort values preserved');
same([[0, 3]], $result->hits[0]->highlights[0]['fragments'][0]['matches'], 'highlight spans preserved');
truth($result->hits[0]->fields['raw'] instanceof Bytes, 'binary stored fields stay Bytes');
same('18446744073709551615', $result->totalHits, 'uint64 total hits remain lossless');
same('LowerBound', $result->totalHitsRelation, 'total hit relation preserved');
same(99.5, $result->coverage?->coveragePercent, 'coverage preserved');
same(true, $result->coverage?->degraded?->timeout, 'coverage degradation preserved');
same(0.02, $result->timing?->searchTimeSeconds, 'timing preserved');
same(true, $result->errors[0]->transient, 'embedded result error preserved');
same('trace', $result->trace, 'trace preserved');
same('local', $result->metadata['server'], 'result metadata preserved');
same(2, $result->aggregations['count'], 'root aggregations derive from root fields');
same('opaque-token', $result->continuations['next'], 'continuation token remains opaque');

// --------------------------------------------------------------------------
// Portable operation errors.
// --------------------------------------------------------------------------

try {
    Protocol::expectPayload([
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_Error',
        'payload' => [
            'code' => 'InvalidArgument',
            'message' => 'bad request',
            'retryable' => false,
            'details' => [['code' => 'unknown_field', 'path' => 'query', 'message' => 'bad']],
        ],
    ], 'SearchResult');
    throw new RuntimeException('portable error did not throw');
} catch (SearchKernelException $error) {
    same('InvalidArgument', $error->errorCode, 'portable error code');
    same('unknown_field', $error->details[0]['code'], 'portable error detail');
}

throws(
    \InvalidArgumentException::class,
    static fn () => Protocol::value(INF),
    'non-finite input is not silently encoded as Float64Value',
);


// --------------------------------------------------------------------------
// CTO adversarial regressions: malformed wire + lossless host representation.
// --------------------------------------------------------------------------

throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'BoolValue',
        'kind' => ['value' => 'false'],
    ]),
    'BoolValue rejects non-boolean payloads instead of PHP truthiness coercion',
);
throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'Float64Value',
        'kind' => ['value' => 'not-a-number'],
    ]),
    'Float64Value rejects non-numeric payloads instead of coercing to zero',
);
throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'StringValue',
        'kind' => ['value' => ['not', 'a', 'string']],
    ]),
    'StringValue rejects non-string payloads',
);
throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'NullValueValue',
        'kind' => ['value' => 'Unspecified'],
    ]),
    'NullValueValue requires the canonical Null enum member',
);
throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'TensorValue',
        'kind' => [
            'type_spec' => 'tensor<float>(x[1])',
            'payload_type' => 'DenseTensorValues',
            'payload' => ['values' => ['not-a-number']],
        ],
    ]),
    'tensor decoding validates the inner payload instead of trusting the union tag',
);
throws(
    \UnexpectedValueException::class,
    static fn () => Protocol::expectPayload([
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_Error',
        'payload' => [
            'code' => ['not-a-code'],
            'message' => 'bad request',
            'retryable' => false,
            'details' => [],
        ],
    ], 'SearchResult'),
    'malformed canonical Error payloads fail as protocol errors',
);
throws(
    \UnexpectedValueException::class,
    static fn () => new Coverage(['full' => ['value' => 'false']]),
    'optional bool result fields reject string coercion',
);

$malformedHit = [
    'node_type' => 'Hit',
    'node' => [
        'id' => ['not-a-string'],
        'fields' => [],
    ],
];
throws(
    \UnexpectedValueException::class,
    static fn () => SearchResult::fromPayload(['root' => $malformedHit]),
    'malformed hit scalar fields fail closed',
);

$encodedEmptyObject = Protocol::value(ValueObject::fromPairs([]));
same('ValueObject', $encodedEmptyObject['kind_type'], 'empty object is distinct from empty list');
same([], $encodedEmptyObject['kind']['fields'], 'empty object encodes with an empty field vector');

$encodedNumericKeyObject = Protocol::value(ValueObject::fromPairs([
    ['key' => '0', 'value' => 'zero'],
]));
same('0', $encodedNumericKeyObject['kind']['fields'][0]['key'], 'numeric-looking string map keys stay strings');

$encodedU64 = Protocol::value(UInt64::fromDecimal('18446744073709551615'));
same('UInt64Value', $encodedU64['kind_type'], 'explicit uint64 uses the UInt64Value union variant');
same('18446744073709551615', $encodedU64['kind']['value'], 'uint64 max remains lossless as decimal text');
$u64Json = Protocol::encodeJson(['value' => $encodedU64]);
truth(
    str_contains($u64Json, '"value":18446744073709551615'),
    'uint64 max is emitted as an unquoted standard FlatBuffers JSON number',
);
truth(
    !str_contains($u64Json, '"18446744073709551615"'),
    'uint64 max is never stringified on the HTTP wire',
);

throws(
    TransportException::class,
    static fn () => new HttpTransport(''),
    'transport configuration errors use the transport error domain',
);
throws(
    ProtocolException::class,
    static fn () => Protocol::expectPayload([], 'SearchResult'),
    'malformed envelopes use the protocol error domain',
);

$statusCode = new ReflectionMethod(HttpTransport::class, 'statusCode');
same(
    200,
    $statusCode->invoke(null, ['HTTP/1.1 302 Found', 'HTTP/1.1 200 OK']),
    'HTTP transport uses the final response status after redirects/proxy hops',
);


// --------------------------------------------------------------------------
// Adversarial wire regressions: malformed data must fail closed.
// --------------------------------------------------------------------------

throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValue(['kind_type' => 'BoolValue', 'kind' => ['value' => 'false']]),
    'BoolValue rejects string coercion',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValue(['kind_type' => 'Float64Value', 'kind' => ['value' => 'not-a-number']]),
    'Float64Value rejects string coercion',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValue(['kind_type' => 'StringValue', 'kind' => ['value' => 7]]),
    'StringValue rejects non-string payload',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValue(['kind_type' => 'NullValueValue', 'kind' => ['value' => 'FutureNull']]),
    'NullValueValue validates its enum payload',
);

$uint64 = new SearchKernel\UInt64('18446744073709551615');
same(
    ['kind_type' => 'UInt64Value', 'kind' => ['value' => '18446744073709551615']],
    Protocol::value($uint64),
    'uint64 max is explicitly representable',
);
throws(
    \InvalidArgumentException::class,
    static fn () => new SearchKernel\UInt64('18446744073709551616'),
    'uint64 overflow rejected',
);

$emptyObject = new SearchKernel\ValueObject();
same('ValueObject', Protocol::value($emptyObject)['kind_type'], 'empty object distinct from empty list');
same('ValueList', Protocol::value([])['kind_type'], 'empty list remains a list');

$oddKeys = new SearchKernel\ValueObject([
    ['key' => '0', 'value' => 'numeric-string-key'],
    ['key' => '😀', 'value' => 'non-bmp'],
]);
$oddWire = Protocol::value($oddKeys);
same(['0', '😀'], array_column($oddWire['kind']['fields'], 'key'), 'explicit object preserves numeric and non-BMP keys');

throws(
    \InvalidArgumentException::class,
    static fn () => Tensor::fromWire('tensor<float>(x[1])', 'DenseTensorValues', ['values' => ['bad']]),
    'dense tensor wire payload validates cell types',
);
throws(
    \InvalidArgumentException::class,
    static fn () => Tensor::fromWire(
        'tensor<float>(x{})',
        'SparseTensorCells',
        ['cells' => [['address' => [['dimension' => 'x', 'label' => 3]], 'value' => 1.0]]],
    ),
    'sparse tensor wire payload validates address entries',
);

throws(
    SearchKernel\ProtocolException::class,
    static fn () => SearchKernelException::fromPayload([
        'code' => 'InvalidArgument',
        'message' => 'bad',
        'retryable' => 'false',
        'details' => [],
    ]),
    'portable Error retryable must be boolean',
);

throws(
    SearchKernel\ProtocolException::class,
    static fn () => SearchKernel\Hit::fromJson(['id' => 7]),
    'hit id must be string',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => SearchKernel\Hit::fromJson([
        'id' => 'doc-1',
        'highlights' => [[
            'field' => 'title',
            'fragments' => [['text' => 'x', 'matches' => [['start_byte' => '0', 'end_byte' => 1]]]],
        ]],
    ]),
    'highlight offsets must be uint32',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => new SearchKernel\Coverage(['full' => ['value' => 'false']]),
    'coverage optional bool fails closed',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => new SearchKernel\SearchTiming(['search_time_seconds' => ['value' => '0.1']]),
    'timing optional float fails closed',
);
throws(
    SearchKernel\TransportException::class,
    static fn () => new SearchKernel\HttpTransport(''),
    'transport configuration errors use transport domain',
);

// --------------------------------------------------------------------------
// Query/grouping facade parity uses the canonical portable request shape.
// --------------------------------------------------------------------------

$typedRequest = $index
    ->query(SearchKernel\Query::contains('title', 'fox'))
    ->group(
        SearchKernel\Group::by('brand')
            ->limit(10)
            ->orderBy(SearchKernel\GroupingOrder::desc(SearchKernel\Agg::count()))
            ->output(SearchKernel\Agg::count()->as('products'))
    )
    ->facet('category')
    ->aggregate(SearchKernel\Agg::avg('price')->as('avg_price'))
    ->request();

same('TypedQuery', $typedRequest['query']['source_type'], 'typed query request source');
same('QueryContains', $typedRequest['query']['source']['query']['expr_type'], 'typed contains discriminator');
same(3, count($typedRequest['grouping']), 'group + facet + root aggregate lower to grouping requests');
same('StructuredGroupingSource', $typedRequest['grouping'][0]['source_type'], 'structured grouping source');

echo "PHP client semantic-spine tests PASS\n";
