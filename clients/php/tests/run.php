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
use SearchKernel\Client;
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

$invalidButRepresentable = Protocol::envelope(
    'OpenIndexRequest',
    SearchKernel\JsonObject::empty(),
    'Products',
);
same(
    ['name' => 'Products'],
    $invalidButRepresentable['target'],
    'domain-invalid target reaches canonical semantic validation',
);

/** @param array<string, mixed> $overrides @return array<string, mixed> */
function canonicalResultPayload(array $overrides = []): array
{
    return array_replace([
        'root' => ['node_type' => 'ResultGroup', 'node' => ['children' => []]],
        'total_hits' => ['value' => 0, 'relation' => 'Exact'],
    ], $overrides);
}

final class FixedResponseTransport implements Transport
{
    /** @param array<string,mixed> $response */
    public function __construct(private readonly array $response) {}

    /** @param array<string,mixed> $request
     *  @return array<string,mixed>
     */
    public function execute(array $request): array
    {
        return $this->response;
    }
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
            'SearchKernel_Wire_InsertRequest' => [
                'payload_type' => 'SearchKernel_Wire_InsertResult',
                'payload' => ['id' => $payload['document']['id'] ?? ''],
            ],
            'SearchKernel_Wire_UpsertRequest' => [
                'payload_type' => 'SearchKernel_Wire_UpsertResult',
                'payload' => ['id' => $payload['document']['id'] ?? ''],
            ],
            'SearchKernel_Wire_GetRequest' => [
                'payload_type' => 'SearchKernel_Wire_GetResult',
                'payload' => ['document' => null],
            ],
            'SearchKernel_Wire_DeleteRequest' => [
                'payload_type' => 'SearchKernel_Wire_DeleteResult',
                'payload' => ['id' => $payload['id'] ?? '', 'deleted' => true],
            ],
            'SearchKernel_Wire_PatchRequest' => [
                'payload_type' => 'SearchKernel_Wire_PatchResult',
                'payload' => ['id' => $payload['id'] ?? ''],
            ],
            'SearchKernel_Wire_UpdateRequest' => [
                'payload_type' => 'SearchKernel_Wire_UpdateResult',
                'payload' => ['id' => $payload['id'] ?? ''],
            ],
            'SearchKernel_Wire_SearchRequest' => [
                'payload_type' => 'SearchKernel_Wire_SearchResult',
                'payload' => [
                    'root' => null,
                    'total_hits' => ['value' => 0, 'relation' => 'Exact'],
                ],
            ],
            'SearchKernel_Wire_DescribeRequest' => [
                'payload_type' => 'SearchKernel_Wire_DescribeResult',
                'payload' => ['index' => ['name' => 'test']],
            ],
            'SearchKernel_Wire_AnalyzeRequest' => [
                'payload_type' => 'SearchKernel_Wire_AnalyzeResult',
                'payload' => ['tokens' => [[
                    'surface' => 'Running',
                    'term' => 'run',
                    'position' => 0,
                    'start_byte' => 0,
                    'end_byte' => 7,
                ]]],
            ],
            'SearchKernel_Wire_InspectQueryRequest' => [
                'payload_type' => 'SearchKernel_Wire_InspectQueryResult',
                'payload' => ['resolved_request' => $payload['request'] ?? []],
            ],
            'SearchKernel_Wire_CreateIndexRequest' => [
                'payload_type' => 'SearchKernel_Wire_CreateIndexResult',
                'payload' => ['index' => $payload['definition'] ?? []],
            ],
            'SearchKernel_Wire_OpenIndexRequest' => [
                'payload_type' => 'SearchKernel_Wire_OpenIndexResult',
                'payload' => ['index' => ['fields' => []]],
            ],
            'SearchKernel_Wire_DeleteIndexRequest' => [
                'payload_type' => 'SearchKernel_Wire_DeleteIndexResult',
                'payload' => [],
            ],
            default => throw new RuntimeException('unexpected test request type: ' . $requestType),
        };

        return [
            'protocol_version' => 1,
            ...$result,
        ];
    }
}

same(
    '{"list":[],"object":{}}',
    Protocol::encodeJson([
        'list' => [],
        'object' => SearchKernel\JsonObject::empty(),
    ]),
    'wire JSON preserves empty-list versus empty-object shape',
);
same(
    '{"kind_type":"PositionDataType","kind":{}}',
    Protocol::encodeJson(SearchKernel\Field::position()->dataType()),
    'position data type emits an empty table object, not an array',
);
throws(
    ProtocolException::class,
    static fn () => Protocol::encodeJson(['object' => new stdClass()]),
    'arbitrary PHP objects remain rejected from wire JSON',
);

// --------------------------------------------------------------------------
// Portable value boundary.
// --------------------------------------------------------------------------

$handleTransport = new RecordingTransport();
$firstHandle = new Index($handleTransport, 'products');
$secondHandle = new Index($handleTransport, 'products');
$firstHandle->close();
throws(
    SearchKernel\TransportException::class,
    static fn () => $firstHandle->describe(),
    'closed PHP handle rejects further operations',
);
same('test', $secondHandle->describe()['name'], 'closing one PHP handle leaves another selected handle usable');


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
same((string) PHP_INT_MAX, $document['fields'][1]['value']['kind']['value'], 'int64 JSON uses decimal text');
same('Float64Value', $document['fields'][3]['value']['kind_type'], 'floats stay float64');
same('ValueList', $document['fields'][4]['value']['kind_type'], 'lists stay lists');

$decoded = Protocol::decodeDocument($document);
same('doc-1', $decoded['_id'], 'decode document id');
same(PHP_INT_MAX, $decoded['count'], 'decode int');
same(['one', 'two'], $decoded['tags'], 'decode list');
truth($decoded['meta'] instanceof ValueObject, 'decode object preserves ValueObject identity');
same(
    [
        ['key' => 'a', 'value' => 1],
        ['key' => 'z', 'value' => 2],
    ],
    $decoded['meta']->entries,
    'decode object preserves canonical key/value entries',
);

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
$emptyTensor = Protocol::value(Tensor::dense('tensor<float>(x[2])', []));
$emptyTensorJson = Protocol::encodeJson($emptyTensor);
same(
    '{"kind_type":"TensorValue","kind":{"type_spec":"tensor<float>(x[2])","payload_type":"DenseTensorValues","payload":{}}}',
    $emptyTensorJson,
    'empty dense tensor omits its vector and preserves an empty table for BFBS JSON alignment',
);
$decodedEmptyTensor = Protocol::decodeValue(
    json_decode($emptyTensorJson, true, 512, JSON_THROW_ON_ERROR),
);
truth($decodedEmptyTensor instanceof Tensor, 'empty dense tensor decodes from omitted vector');
same([], $decodedEmptyTensor->payload['values'], 'omitted dense tensor vector decodes as empty');

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

// Facade response fields are part of the protocol boundary and must fail
// closed rather than inheriting PHP scalar coercion rules.
$malformedFacadeCases = [
    ['InsertResult', static fn (Index $i) => $i->insert(['_id' => 'doc']), ['id' => 7]],
    ['UpsertResult', static fn (Index $i) => $i->upsert(['_id' => 'doc']), ['id' => false]],
    ['PatchResult', static fn (Index $i) => $i->patch('doc')->execute(), ['id' => ['doc']]],
    ['UpdateResult', static fn (Index $i) => $i->update('doc')->execute(), ['id' => null]],
    ['DeleteResult', static fn (Index $i) => $i->delete('doc'), ['id' => 'doc', 'deleted' => 'false']],
];
foreach ($malformedFacadeCases as [$resultType, $operation, $payload]) {
    $indexWithMalformedResult = new Index(new FixedResponseTransport([
        'protocol_version' => 1,
        'payload_type' => 'SearchKernel_Wire_' . $resultType,
        'payload' => $payload,
    ]));
    throws(
        ProtocolException::class,
        static fn () => $operation($indexWithMalformedResult),
        $resultType . ' malformed scalar fields fail closed',
    );
}

$getFieldCases = [
    'duplicate key' => [
        ['key' => 'same', 'value' => Protocol::value(1)],
        ['key' => 'same', 'value' => Protocol::value(2)],
    ],
    'null entry' => [null],
    'missing key' => [['value' => Protocol::value(1)]],
    'missing value' => [['key' => 'missing']],
];
foreach ($getFieldCases as $label => $fields) {
    $malformedGet = new Index(new FixedResponseTransport([
        'protocol_version' => 1,
        'payload_type' => 'SearchKernel_Wire_GetResult',
        'payload' => ['document' => ['id' => 'doc-1', 'fields' => $fields]],
    ]));
    throws(
        ProtocolException::class,
        static fn () => $malformedGet->get('doc-1'),
        'GET rejects malformed semantic map: ' . $label,
    );
}
$validGet = new Index(new FixedResponseTransport([
    'protocol_version' => 1,
    'payload_type' => 'SearchKernel_Wire_GetResult',
    'payload' => ['document' => [
        'id' => 'doc-1',
        'fields' => [
            ['key' => 'nothing', 'value' => Protocol::value(null)],
            ['key' => 'count', 'value' => Protocol::value(3)],
        ],
    ]],
]));
same(
    ['nothing' => null, 'count' => 3, '_id' => 'doc-1'],
    $validGet->get('doc-1'),
    'GET preserves explicit canonical null and valid semantic-map values',
);

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
same('SearchKernel_Wire_PatchRequest', $patch['payload_type'], 'patch envelope discriminator');
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

same(
    'doc-1',
    $index->update('doc-1')
        ->matchElement('people', 'alice', SearchKernel\MatchedUpdate::increment(2))
        ->tensorModify(
            'embedding',
            'replace',
            Tensor::sparse('tensor<float>(x{})', [[
                'address' => ['x' => 'alice'],
                'value' => 0.75,
            ]]),
            false,
        )
        ->execute(),
    'advanced update result id',
);
$advancedUpdate = $transport->requests[array_key_last($transport->requests)];
same('MatchUpdate', $advancedUpdate['payload']['operations'][0]['operation_type'], 'match update discriminator');
same(
    'ArithmeticUpdate',
    $advancedUpdate['payload']['operations'][0]['operation']['update']['operation_type'],
    'matched arithmetic discriminator',
);
same(
    'Increment',
    $advancedUpdate['payload']['operations'][0]['operation']['update']['operation']['operator'],
    'matched arithmetic operator',
);
same('TensorModifyUpdate', $advancedUpdate['payload']['operations'][1]['operation_type'], 'tensor modify discriminator');
same('Replace', $advancedUpdate['payload']['operations'][1]['operation']['operator'], 'tensor modify operator');
same('SparseTensorCells', $advancedUpdate['payload']['operations'][1]['operation']['payload_type'], 'tensor modify payload union');
same(['value' => false], $advancedUpdate['payload']['operations'][1]['operation']['create'], 'tensor modify create presence');

throws(
    \InvalidArgumentException::class,
    static fn () => $index->update('doc-1')->tensorModify(
        'embedding',
        'replace',
        Tensor::dense('tensor<float>(x[1])', [1.0]),
    ),
    'tensor modify rejects dense payloads not represented by the canonical union',
);

// --------------------------------------------------------------------------
// Search request facade and canonical namespaces.
// --------------------------------------------------------------------------

throws(
    ArgumentCountError::class,
    static fn () => $index->search('fox')->sort('price'),
    'field sort direction is required',
);

$request = $index
    ->search('brown fox')
    ->searchField('title', 3.0)
    ->searchField('description')
    ->filter('price <= :value', ['value' => 120])
    ->filter('rating >= :value', ['value' => 4])
    ->sort('price', 'desc', 'last')
    ->sortRelevance()
    ->fields()
    ->summary('default')
    ->highlight('title')
    ->ranking('bm25')
    ->rankProperty('foo', '123')
    ->input('embedding', Tensor::dense('tensor<float>(x[2])', [1, 2]))
    ->property('trace.level', 2)
    ->page(2, 20)
    ->timeout(1000)
    ->request();

same('SearchKernelTextQuery', $request['query']['source_type'], 'search query source');
same('title', $request['search_fields']['fields'][0]['field'], 'weighted search field');
same(['value' => 3.0], $request['search_fields']['fields'][0]['weight'], 'search field weight presence');
same('description', $request['search_fields']['fields'][1]['field'], 'unweighted search field');
truth(
    !array_key_exists('weight', $request['search_fields']['fields'][1]),
    'absent search-field weight stays absent',
);
same(2, count($request['filters']['textual']), 'repeated filters remain separate clauses');
same('value', $request['filters']['textual'][0]['bindings'][0]['key'], 'first clause binding key');
same('120', $request['filters']['textual'][0]['bindings'][0]['value']['kind']['value'], 'first clause binding value');
same('4', $request['filters']['textual'][1]['bindings'][0]['value']['kind']['value'], 'second clause binding value');
same('FieldSortTarget', $request['sort'][0]['target']['target_type'], 'field sort target');
same('Desc', $request['sort'][0]['direction'], 'field sort direction');
same('MissingLast', $request['sort'][0]['missing']['policy_type'], 'missing policy');
same('RelevanceSortTarget', $request['sort'][1]['target']['target_type'], 'relevance sort target');
same([], $request['presentation']['fields']['fields'], 'present-empty projection survives search builder');
same(['title'], $request['presentation']['highlighting']['fields'], 'highlight fields');
same('bm25', $request['ranking']['profile'], 'ranking profile');
same([['key' => 'foo', 'value' => '123']], $request['ranking']['properties'], 'rank property namespace');
foreach ([false, null, 7, [], new stdClass()] as $rankPropertyValue) {
    throws(
        TypeError::class,
        static fn () => $index->search('fox')->rankProperty('enabled', $rankPropertyValue),
        'rank property values must be strings',
    );
}
throws(
    TypeError::class,
    static fn () => $index->search('fox')->rankProperties(['enabled' => false]),
    'rankProperties rejects non-string values',
);
same('LiteralQueryInput', $request['inputs'][0]['input_type'], 'query input namespace');
same('TensorValue', $request['inputs'][0]['input']['value']['kind_type'], 'tensor query input');
same([['key' => 'trace.level', 'value' => Protocol::value(2)]], $request['properties'], 'generic property namespace');
same(['value' => 20], $request['offset'], 'page offset');
same(['value' => 20], $request['limit'], 'page limit');
same(['value' => '1000'], $request['timeout_ms'], 'timeout uses canonical uint64 decimal text');

same(
    'MatchAllQuery',
    $index->browse()->request()['query']['source_type'],
    'browse uses explicit match-all query source',
);
$yql = $index->yql('select * from sources * where title contains text(@q)', ['q' => 'fox'])->request();
same('YqlQuery', $yql['query']['source_type'], 'YQL source discriminator');
same('q', $yql['query']['source']['parameters'][0]['key'], 'YQL parameter namespace');

$advanced = $index->search('fox')
    ->inputExpr('embedded_query', 'embed(transformer,@query)')
    ->rankProperties(['alpha' => 'one', 'beta' => 'two'])
    ->properties(['trace.level' => 2, 'custom.flag' => true])
    ->rawGrouping('all(output(count()))')
    ->format('json')
    ->bolding(false)
    ->timing(true)
    ->language('en')
    ->locale('en-US')
    ->defaultIndex('title')
    ->sources('products')
    ->restrict('product')
    ->searchPath('/0')
    ->searchGroup(2)
    ->queryString('fox')
    ->queryModelType('weakAnd', 'linguistics', 'advanced', 'default', true)
    ->queryProfile('web')
    ->searchChain('default')
    ->request();

$implicitRanking = $index->search('fox')->rankProperty('source', 'catalog')->request()['ranking'];
same(false, array_key_exists('profile', $implicitRanking), 'rank properties preserve absent ranking profile');
same([['key' => 'source', 'value' => 'catalog']], $implicitRanking['properties'], 'rank properties remain present');
$explicitRanking = $index->search('fox')->ranking('default')->rankProperty('source', 'catalog')->request()['ranking'];
same('default', $explicitRanking['profile'], 'explicit default ranking profile remains explicit');

$advancedInputs = [];
foreach ($advanced['inputs'] as $input) {
    $advancedInputs[$input['name']] = $input;
}
same('ExpressionQueryInput', $advancedInputs['embedded_query']['input_type'], 'expression input discriminator');
same('embed(transformer,@query)', $advancedInputs['embedded_query']['input']['expression'], 'expression input payload');
same('RawGroupingSource', $advanced['grouping'][0]['source_type'], 'raw grouping discriminator');
same('all(output(count()))', $advanced['grouping'][0]['source']['expression'], 'raw grouping source');
same('json', $advanced['presentation']['format'], 'presentation format');
same(['value' => false], $advanced['presentation']['bolding'], 'presentation bolding presence');
same(['value' => true], $advanced['presentation']['timing'], 'presentation timing presence');
same('en', $advanced['query_model']['language'], 'query-model language');
same('en-US', $advanced['query_model']['locale'], 'query-model locale');
same('title', $advanced['query_model']['default_index'], 'query-model default index');
same(['products'], $advanced['query_model']['sources'], 'query-model sources');
same(['product'], $advanced['query_model']['restrict'], 'query-model restrict');
same('/0', $advanced['query_model']['search_path'], 'query-model search path');
same(['value' => 2], $advanced['query_model']['search_group'], 'query-model search group');
same('fox', $advanced['query_model']['query_string'], 'query-model query string');
same(['value' => true], $advanced['query_model']['type']['is_yql_default'], 'query-model type optional bool');
same('web', $advanced['query_profile'], 'query profile');
same('default', $advanced['search_chain'], 'search chain');
same(['alpha', 'beta'], array_column($advanced['ranking']['properties'], 'key'), 'rank properties canonical order');
same(['custom.flag', 'trace.level'], array_column($advanced['properties'], 'key'), 'generic properties canonical order');

$facetsRequest = $index->browse()->facets('brand', 'category')->request();
same(2, count($facetsRequest['grouping']), 'facets convenience creates one grouping request per field');

$yqlBuilder = $index->yql('select * from sources * where title contains text(@q)')
    ->property('q', 'fox')
    ->ranking('default')
    ->inputExpr('embedded_query', 'embed(transformer,@query)')
    ->rawGrouping('all(output(count()))')
    ->limit(10);
truth(!method_exists($yqlBuilder, 'filter'), 'YQL builder does not expose high-level filter');
truth(!method_exists($yqlBuilder, 'sort'), 'YQL builder does not expose high-level sort');
truth(!method_exists($yqlBuilder, 'searchField'), 'YQL builder does not expose search-field selection');
truth(!method_exists($yqlBuilder, 'fields'), 'YQL builder does not expose projection');
$restrictedYql = $yqlBuilder->request();
same('YqlQuery', $restrictedYql['query']['source_type'], 'restricted YQL remains YQL');
same('ExpressionQueryInput', $restrictedYql['inputs'][0]['input_type'], 'YQL expression input is request-level');
same('RawGroupingSource', $restrictedYql['grouping'][0]['source_type'], 'YQL programmatic grouping is request-level');
same(['value' => 10], $restrictedYql['limit'], 'YQL request-level pagination');

$resolved = $index->search('inspect me')->limit(5)->inspect();
same('SearchKernelTextQuery', $resolved['query']['source_type'], 'inspect returns server-resolved request');
same(['value' => 5], $resolved['limit'], 'inspect sends configured search controls');
$inspectRequest = $transport->requests[array_key_last($transport->requests)];
same('SearchKernel_Wire_InspectQueryRequest', $inspectRequest['payload_type'], 'inspect request discriminator');

$tokens = $index->analyze('Running')->field('title')->language('en')->execute();
same('run', $tokens[0]['term'], 'analyze result term');
same(7, $tokens[0]['end_byte'], 'analyze result byte span');
$analyzeRequest = $transport->requests[array_key_last($transport->requests)];
same('SearchKernel_Wire_AnalyzeRequest', $analyzeRequest['payload_type'], 'analyze request discriminator');
same('AnalyzeFieldTarget', $analyzeRequest['payload']['target']['target_type'], 'analyze field target');
same('en', $analyzeRequest['payload']['language'], 'analyze language');

$badAnalyze = new Index(new class implements Transport {
    public function execute(array $request): array
    {
        return [
            'protocol_version' => 1,
            'payload_type' => 'SearchKernel_Wire_AnalyzeResult',
            'payload' => ['tokens' => [[
                'surface' => 'bad',
                'term' => 'bad',
                'position' => '0',
                'start_byte' => 0,
                'end_byte' => 3,
            ]]],
        ];
    }
});
throws(
    ProtocolException::class,
    static fn () => $badAnalyze->analyze('bad')->execute(),
    'malformed analyze token fails closed',
);

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
        'protocol_version' => 1,
        'payload_type' => 'SearchKernel_Wire_Error',
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
        'protocol_version' => 1,
        'payload_type' => 'SearchKernel_Wire_Error',
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
    static fn () => SearchResult::fromPayload(canonicalResultPayload(['root' => $malformedHit])),
    'malformed hit scalar fields fail closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchResult::fromPayload([]),
    'missing SearchResult root fails closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchResult::fromPayload([
        'root' => ['node_type' => 'ResultGroup', 'node' => ['children' => []]],
        'total_hits' => null,
    ]),
    'null SearchResult total_hits fails closed',
);

$encodedEmptyObject = Protocol::value(ValueObject::fromPairs([]));
same('ValueObject', $encodedEmptyObject['kind_type'], 'empty object is distinct from empty list');
same([], $encodedEmptyObject['kind']['fields'], 'empty object encodes with an empty field vector');
$decodedEmptyObject = Protocol::decodeValue($encodedEmptyObject);
truth($decodedEmptyObject instanceof ValueObject, 'decoded ValueObject retains its object type when empty');
same([], $decodedEmptyObject->entries, 'empty decoded ValueObject has no entries');
$decodedObject = Protocol::decodeValue(Protocol::value(ValueObject::fromPairs([
    ['key' => 'name', 'value' => 'searchkernel'],
])));
truth($decodedObject instanceof ValueObject, 'decoded non-empty ValueObject retains its object type');
same('name', $decodedObject->entries[0]['key'], 'decoded ValueObject preserves entry keys');
same('searchkernel', $decodedObject->entries[0]['value'], 'decoded ValueObject preserves entry values');
$decodedEmptyList = Protocol::decodeValue(['kind_type' => 'ValueList', 'kind' => ['values' => []]]);
same([], $decodedEmptyList, 'decoded empty ValueList remains a PHP list');
truth(!$decodedEmptyList instanceof ValueObject, 'decoded empty ValueList stays distinct from ValueObject');

$encodedNumericKeyObject = Protocol::value(ValueObject::fromPairs([
    ['key' => '0', 'value' => 'zero'],
]));
same('0', $encodedNumericKeyObject['kind']['fields'][0]['key'], 'numeric-looking string map keys stay strings');

$encodedU64 = Protocol::value(UInt64::fromDecimal('18446744073709551615'));
same('UInt64Value', $encodedU64['kind_type'], 'explicit uint64 uses the UInt64Value union variant');
same('18446744073709551615', $encodedU64['kind']['value'], 'uint64 max remains lossless as decimal text');
$u64Json = Protocol::encodeJson(['value' => $encodedU64]);
truth(
    str_contains($u64Json, '"value":"18446744073709551615"'),
    'uint64 max is emitted as a canonical decimal string on the HTTP wire',
);
truth(
    !str_contains($u64Json, '"value":18446744073709551615'),
    'uint64 max is never emitted as a bare JSON number',
);

$i64MinJson = Protocol::encodeJson(['value' => Protocol::value(PHP_INT_MIN)]);
$i64MaxJson = Protocol::encodeJson(['value' => Protocol::value(PHP_INT_MAX)]);
truth(
    str_contains($i64MinJson, '"value":"-9223372036854775808"'),
    'int64 min is emitted as canonical decimal text',
);
truth(
    str_contains($i64MaxJson, '"value":"9223372036854775807"'),
    'int64 max is emitted as canonical decimal text',
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
    ProtocolException::class,
    static fn () => Tensor::fromWire('tensor<float>(x[1])', 'DenseTensorValues', ['values' => ['bad']]),
    'dense tensor wire payload validates cell types in the protocol error domain',
);
throws(
    ProtocolException::class,
    static fn () => Tensor::fromWire(
        'tensor<float>(x{})',
        'SparseTensorCells',
        ['cells' => [['address' => [['dimension' => 'x', 'label' => 3]], 'value' => 1.0]]],
    ),
    'sparse tensor wire payload validates address entries in the protocol error domain',
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
throws(
    SearchKernel\TransportException::class,
    static fn () => new SearchKernel\HttpTransport('http://127.0.0.1', 0.0),
    'transport timeout must be positive',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'ValueList',
        'kind' => ['values' => [
            'not-a-vector' => ['kind_type' => 'StringValue', 'kind' => ['value' => 'x']],
        ]],
    ]),
    'ValueList rejects object-shaped vectors',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'BytesValue',
        'kind' => ['value' => ['first' => 1]],
    ]),
    'BytesValue rejects object-shaped byte vectors',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => Protocol::decodeValueEntries([
        'field' => [
            'key' => 'title',
            'value' => ['kind_type' => 'StringValue', 'kind' => ['value' => 'x']],
        ],
    ]),
    'ValueEntry vectors reject object-shaped JSON',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => SearchKernel\SearchResult::fromPayload(canonicalResultPayload([
        'errors' => ['only' => ['code' => 1]],
    ])),
    'SearchResult errors reject object-shaped vectors',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => SearchKernel\SearchResult::fromPayload(canonicalResultPayload([
        'root' => [
            'node_type' => 'ResultGroup',
            'node' => [
                'limits' => ['from' => 'not-a-value'],
                'children' => [],
            ],
        ],
    ])),
    'present malformed group limits fail closed instead of becoming null',
);
throws(
    SearchKernel\ProtocolException::class,
    static fn () => SearchKernel\Hit::fromJson([
        'id' => 'doc',
        'highlights' => ['title' => ['field' => 'title', 'fragments' => []]],
    ]),
    'hit highlight vectors reject object-shaped JSON',
);
truth(
    class_exists(SearchKernel\CoverageDegraded::class),
    'CoverageDegraded is independently autoloadable through its PSR-4 path',
);

// --------------------------------------------------------------------------
// FlatBuffers JSON printer default-scalar semantics.
// --------------------------------------------------------------------------

$defaultDeleteIndex = new Index(new FixedResponseTransport([
    'protocol_version' => 1,
    'payload_type' => 'SearchKernel_Wire_DeleteResult',
    'payload' => ['id' => 'doc-default'],
]));
same(false, $defaultDeleteIndex->delete('doc-default'), 'omitted DeleteResult.deleted defaults false');

$defaultError = SearchKernelException::fromPayload([
    'code' => 'InvalidArgument',
    'message' => 'not retryable',
    'details' => [],
]);
same(false, $defaultError->retryable, 'omitted Error.retryable defaults false');

$defaultAnalyzeIndex = new Index(new FixedResponseTransport([
    'protocol_version' => 1,
    'payload_type' => 'SearchKernel_Wire_AnalyzeResult',
    'payload' => ['tokens' => [[
        'surface' => 'fox',
        'term' => 'fox',
        'end_byte' => 3,
    ]]],
]));
$defaultTokens = $defaultAnalyzeIndex->analyze('fox')->execute();
same(0, $defaultTokens[0]['position'], 'omitted AnalyzedToken.position defaults zero');
same(0, $defaultTokens[0]['start_byte'], 'omitted AnalyzedToken.start_byte defaults zero');
same(3, $defaultTokens[0]['end_byte'], 'present AnalyzedToken.end_byte preserved');

$defaultHighlight = SearchKernel\Hit::fromJson([
    'id' => 'doc-default',
    'highlights' => [[
        'field' => 'title',
        'fragments' => [[
            'text' => 'fox',
            'matches' => [['end_byte' => 3]],
        ]],
    ]],
]);
same(
    [[0, 3]],
    $defaultHighlight->highlights[0]['fragments'][0]['matches'],
    'omitted HighlightSpan.start_byte defaults zero',
);

same(
    0.0,
    Protocol::decodeRelevance([
        'value_type' => 'FiniteRelevance',
        'value' => [],
    ]),
    'omitted FiniteRelevance.value defaults zero',
);

$defaultSparseTensor = Tensor::fromWire(
    'tensor<float>(x{})',
    'SparseTensorCells',
    ['cells' => [[
        'address' => [['dimension' => 'x', 'label' => 'alice']],
    ]]],
);
same(
    0.0,
    $defaultSparseTensor->payload['cells'][0]['value'],
    'omitted TensorCell.value defaults zero',
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

$advancedGroup = SearchKernel\Group::by(
    SearchKernel\G::fn('time.year', SearchKernel\G::field('published_at')),
)
    ->unlimited()
    ->precision(100)
    ->alias('revenue', SearchKernel\Agg::sum('price'))
    ->orderBy(SearchKernel\GroupingOrder::desc(SearchKernel\G::aliasRef('revenue')))
    ->filter(SearchKernel\Gf::range(SearchKernel\G::field('price'), 10, 100))
    ->keep(SearchKernel\Gf::regex(SearchKernel\G::field('brand'), '^A'))
    ->where(SearchKernel\G::field('in_stock'))
    ->output(
        SearchKernel\G::aliasRef('revenue')->as('revenue'),
        SearchKernel\Agg::xor('flags')->as('flags_xor'),
        SearchKernel\Agg::quantiles('price', 0.5, 0.9)->as('price_quantiles'),
        SearchKernel\Agg::count()->multiply(2)->as('double_count'),
    )
    ->continuation('opaque-token')
    ->toRequest();

same('StructuredGroupingSource', $advancedGroup['source_type'], 'advanced grouping remains structured');
same(['opaque-token'], $advancedGroup['continuations'], 'grouping continuation remains opaque');
same(
    'GroupingFunctionExpression',
    $advancedGroup['source']['root']['operations'][0]['operation']['expression']['expr_type'],
    'grouping function expression',
);
$advancedGroupBody = $advancedGroup['source']['root']['operations'][1]['operation']['operations'];
$advancedGroupKinds = array_column($advancedGroupBody, 'operation_type');
same(
    [
        'GroupingAliasOperation',
        'GroupingMaxOperation',
        'GroupingPrecisionOperation',
        'GroupingOrderOperation',
        'GroupingFilterOperation',
        'GroupingKeepOperation',
        'GroupingWhereOperation',
        'GroupingOutputOperation',
    ],
    $advancedGroupKinds,
    'advanced grouping operation ordering',
);
same(
    'InfiniteGroupingMax',
    $advancedGroupBody[1]['operation']['value_type'],
    'explicit unlimited group uses infinite max rather than omitting max',
);
same(
    'GroupingFilterRange',
    $advancedGroupBody[4]['operation']['filter']['expr_type'],
    'grouping filter range',
);
same(
    'GroupingFilterRegex',
    $advancedGroupBody[5]['operation']['filter']['expr_type'],
    'grouping keep regex',
);
same(
    'GroupingBinaryExpression',
    $advancedGroupBody[7]['operation']['outputs'][3]['expression']['expr_type'],
    'grouping arithmetic output',
);
same(
    'QuantilesAggregation',
    $advancedGroupBody[7]['operation']['outputs'][2]['expression']['expr']['aggregation_type'],
    'quantiles aggregation',
);

$facetPrecision = $index->browse()
    ->facet('brand', 'brands', 20, 'value_asc', 200)
    ->request();
$facetBody = $facetPrecision['grouping'][0]['source']['root']['operations'][1]['operation']['operations'];
same('GroupingPrecisionOperation', $facetBody[1]['operation_type'], 'facet precision lowers explicitly');

$queryExpr = static function (array $query): array {
    $expr = $query['source']['query'] ?? null;
    if (!is_array($expr)) {
        throw new RuntimeException('expected typed query helper result');
    }
    return $expr;
};

$phrase = SearchKernel\Query::contains(
    'title',
    SearchKernel\Query::phrase('wireless', 'headphones'),
);
same('QueryContains', $queryExpr($phrase)['expr_type'], 'contains typed query');
same('QueryPhrase', $queryExpr($phrase)['expr']['item']['expr_type'], 'phrase query item');

$boolean = SearchKernel\Query::allOf(
    SearchKernel\Query::eq('brand', 'Acme'),
    SearchKernel\Query::negate(SearchKernel\Query::alwaysFalse()),
);
same('QueryAnd', $queryExpr($boolean)['expr_type'], 'boolean AND query');
same('QueryComparison', $queryExpr($boolean)['expr']['clauses'][0]['expr_type'], 'comparison query');
same('QueryNot', $queryExpr($boolean)['expr']['clauses'][1]['expr_type'], 'boolean NOT query');

$range = SearchKernel\Query::range('price', gte: 10, lt: 100);
same('QueryRange', $queryExpr($range)['expr_type'], 'range query');
same('FiniteRangeBound', $queryExpr($range)['expr']['lower']['bound_type'], 'range lower bound');
same(['value' => true], $queryExpr($range)['expr']['include_lower'], 'range lower inclusion');
same(['value' => false], $queryExpr($range)['expr']['include_upper'], 'range upper exclusion');

same('QueryIn', $queryExpr(SearchKernel\Query::inValues('brand', ['Acme', 'TrailCo']))['expr_type'], 'IN query');
same('QueryNear', $queryExpr(SearchKernel\Query::near('one', 'two'))['expr_type'], 'near query');
same('QueryONear', $queryExpr(SearchKernel\Query::onear('one', 'two'))['expr_type'], 'ordered-near query');
same(
    'QuerySameElement',
    $queryExpr(SearchKernel\Query::sameElement(SearchKernel\Query::contains('first_name', 'Joe')))['expr_type'],
    'same-element query',
);
same(
    'QueryEquiv',
    $queryExpr(SearchKernel\Query::equiv(SearchKernel\Query::term('color'), SearchKernel\Query::term('colour')))['expr_type'],
    'equivalence query',
);
same('QueryFuzzy', $queryExpr(SearchKernel\Query::fuzzy('colour'))['expr_type'], 'fuzzy query');
same('QueryMatches', $queryExpr(SearchKernel\Query::matches('title', 'foo.*'))['expr_type'], 'matches query');
// SKC: query.prefix_suffix_substring/basic kinds=ergonomic runtime=http
$fieldedTerm = $queryExpr(SearchKernel\Query::term('title', 'quick'));
same('QueryContains', $fieldedTerm['expr_type'], 'fielded term query');
same('TermQuery', $fieldedTerm['expr']['item']['expr_type'], 'fielded term canonical item');
foreach ([
    [SearchKernel\Query::prefix('brand', 'A.c+'), 'A\\.c\\+.*'],
    [SearchKernel\Query::substring('brand', 'A.c+'), '.*A\\.c\\+.*'],
    [SearchKernel\Query::suffix('brand', 'A.c+'), '.*A\\.c\\+'],
] as [$literalQuery, $pattern]) {
    $literalExpr = $queryExpr($literalQuery);
    same('QueryMatches', $literalExpr['expr_type'], 'literal query canonical kind');
    same($pattern, $literalExpr['expr']['pattern'], 'literal query escaped pattern');
}
same('QueryText', $queryExpr(SearchKernel\Query::text('hello'))['expr_type'], 'typed text query');
same('QueryUserInput', $queryExpr(SearchKernel\Query::userInput('hello'))['expr_type'], 'user-input query');
same('QueryUserQuery', $queryExpr(SearchKernel\Query::userQuery())['expr_type'], 'user-query node');

same(
    'QueryDotProduct',
    $queryExpr(SearchKernel\Query::dotProduct('tags', ['a' => 10]))['expr_type'],
    'dot-product query',
);
same(
    'QueryWeightedSet',
    $queryExpr(SearchKernel\Query::weightedSet('tags', ['a' => 10]))['expr_type'],
    'weighted-set query',
);
$wand = $queryExpr(SearchKernel\Query::wand('tags', ['b' => 5, 'a' => 10]));
same('QueryWand', $wand['expr_type'], 'WAND query');
same(
    ['a', 'b'],
    array_map(
        static fn (array $item): mixed => $item['key']['kind']['value'] ?? null,
        $wand['expr']['items'],
    ),
    'WAND items are canonical and preserve keys',
);
same(
    ['10', '5'],
    array_column($wand['expr']['items'], 'weight'),
    'WAND weights preserve exact int64 strings',
);
same(
    'QueryWeakAnd',
    $queryExpr(SearchKernel\Query::weakAnd(
        SearchKernel\Query::contains('title', 'fox'),
        SearchKernel\Query::contains('body', 'fox'),
    ))['expr_type'],
    'weak-and query',
);
same(
    'QueryRank',
    $queryExpr(SearchKernel\Query::rank(
        SearchKernel\Query::contains('title', 'fox'),
        SearchKernel\Query::contains('body', 'fox'),
    ))['expr_type'],
    'rank query',
);
same(
    'QueryLabeled',
    $queryExpr(SearchKernel\Query::labeled(SearchKernel\Query::contains('title', 'fox'), 'candidate', 1.5))['expr_type'],
    'labeled query',
);
same(
    'QueryNonEmpty',
    $queryExpr(SearchKernel\Query::nonEmpty(SearchKernel\Query::contains('title', 'fox')))['expr_type'],
    'non-empty query',
);
same(
    'QueryGeoLocation',
    $queryExpr(SearchKernel\Query::geoLocation('location', 37.3, -121.9, '10km'))['expr_type'],
    'geo-location query',
);
same(
    'QueryGeoBoundingBox',
    $queryExpr(SearchKernel\Query::geoBox('location', 37.0, -122.0, 38.0, -121.0))['expr_type'],
    'geo-box query',
);

$nearestAnnotated = SearchKernel\Query::nearestNeighbor(
    'embedding',
    'query_embedding',
    ['approximate' => true, 'targetNumHits' => 20],
);
same('QueryNearestNeighbor', $queryExpr($nearestAnnotated)['expr_type'], 'nearest-neighbor query');
same(
    ['approximate', 'targetNumHits'],
    array_column($queryExpr($nearestAnnotated)['annotations'], 'key'),
    'nearest-neighbor annotations are canonical sorted entries',
);

$predicate = SearchKernel\Query::predicate(
    'targeting',
    ['country' => 'US'],
    ['age' => 18],
);
same('QueryPredicate', $queryExpr($predicate)['expr_type'], 'predicate query');
same('country', $queryExpr($predicate)['expr']['features']['fields'][0]['key'], 'predicate feature map');

throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::range('price', gte: 10, gt: 11),
    'range rejects conflicting lower-bound forms',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::phrase(),
    'phrase rejects an empty term list',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::near(),
    'near rejects an empty term list',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::sameElement(),
    'same-element rejects an empty clause list',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::wand('title', []),
    'WAND rejects an empty weighted term map',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::wand('title', ['fox' => 1.5]),
    'WAND rejects non-integer weights',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::geoLocation('location', INF, -121.9, '10km'),
    'geo-location rejects non-finite coordinates',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::geoBox('location', 37.0, -122.0, NAN, -121.0),
    'geo-box rejects non-finite coordinates',
);
throws(
    \InvalidArgumentException::class,
    static fn () => SearchKernel\Query::annotate(
        SearchKernel\Query::annotate(SearchKernel\Query::term('fox'), ['label' => 'a']),
        ['label' => 'b'],
    ),
    'duplicate query annotations are rejected',
);

/*
Canonical portable request/result conformance evidence.

SKC: query.term/basic kinds=conformance runtime=http
SKC: query.boolean_constants/basic kinds=conformance runtime=http
SKC: query.comparison/basic kinds=conformance runtime=http
SKC: query.range/basic kinds=conformance runtime=http
SKC: query.boolean/basic kinds=conformance runtime=http
SKC: query.phrase/basic kinds=conformance runtime=http
SKC: query.proximity/basic kinds=conformance runtime=http
SKC: query.same_element/basic kinds=conformance runtime=http
SKC: query.fuzzy/basic kinds=conformance runtime=http
SKC: query.nearest_neighbor/basic kinds=conformance runtime=http
SKC: query.geo/basic kinds=conformance runtime=http
SKC: query.predicate/basic kinds=conformance runtime=http
SKC: query.weighted_and_wand/basic kinds=conformance runtime=http
SKC: query.weighted_set_dot_product/basic kinds=conformance runtime=http
SKC: query.equivalence/basic kinds=conformance runtime=http
SKC: query.rank/basic kinds=conformance runtime=http
SKC: query.user_input_user_query/basic kinds=conformance runtime=http
SKC: query.typed_text/basic kinds=conformance runtime=http
SKC: query.in_term/basic kinds=conformance runtime=http
SKC: query.non_empty/basic kinds=conformance runtime=http
SKC: query.labels/basic kinds=conformance runtime=http
SKC: filter.textual/basic kinds=conformance runtime=http
SKC: search.sorting/basic kinds=conformance runtime=http
SKC: search.search_fields/basic kinds=conformance runtime=http
SKC: search.pagination/basic kinds=conformance runtime=http
SKC: grouping.structured/basic kinds=conformance runtime=http
SKC: grouping.functions/basic kinds=conformance runtime=http
SKC: grouping.aggregations/basic kinds=conformance runtime=http
SKC: grouping.continuations/basic kinds=conformance runtime=http
SKC: grouping.raw_source/basic kinds=conformance runtime=http
SKC: update.document_crud/basic kinds=conformance runtime=http
SKC: update.arithmetic/basic kinds=conformance runtime=http
SKC: update.create_if_nonexistent/basic kinds=conformance runtime=http
SKC: update.matched_element/basic kinds=conformance runtime=http
SKC: update.tensor_modify/basic kinds=conformance runtime=http
SKC: query.yql/basic kinds=conformance runtime=http
SKC: ranking.profile_selection/basic kinds=conformance runtime=http
SKC: ranking.tensor_query_inputs/basic kinds=conformance runtime=http
SKC: ranking.rank_properties/basic kinds=conformance runtime=http
SKC: result.highlighting/basic kinds=conformance runtime=http
SKC: result.grouping_tree/basic kinds=conformance runtime=http
SKC: result.degradation_metadata/basic kinds=conformance runtime=http
*/


$expectedOperations = [
    'insert' => ['InsertRequest', 'InsertResult'],
    'upsert' => ['UpsertRequest', 'UpsertResult'],
    'patch' => ['PatchRequest', 'PatchResult'],
    'update' => ['UpdateRequest', 'UpdateResult'],
    'delete' => ['DeleteRequest', 'DeleteResult'],
    'get' => ['GetRequest', 'GetResult'],
    'search' => ['SearchRequest', 'SearchResult'],
    'analyze' => ['AnalyzeRequest', 'AnalyzeResult'],
    'describe' => ['DescribeRequest', 'DescribeResult'],
    'inspect_query' => ['InspectQueryRequest', 'InspectQueryResult'],
    'create_index' => ['CreateIndexRequest', 'CreateIndexResult'],
    'open_index' => ['OpenIndexRequest', 'OpenIndexResult'],
    'delete_index' => ['DeleteIndexRequest', 'DeleteIndexResult'],
];
same(array_keys($expectedOperations), Protocol::operations(), 'low-level portable operation list');
foreach ($expectedOperations as $operation => $types) {
    same($types, Protocol::operation($operation), 'low-level operation mapping: ' . $operation);
}

$lowLevelTransport = new RecordingTransport();
$lowLevel = new Client($lowLevelTransport);
same('doc-low', $lowLevel->insert(['document' => ['id' => 'doc-low', 'fields' => []]])['id'], 'low-level insert');
same('doc-low', $lowLevel->upsert(['document' => ['id' => 'doc-low', 'fields' => []]])['id'], 'low-level upsert');
same('doc-low', $lowLevel->patch(['id' => 'doc-low', 'operations' => []])['id'], 'low-level patch');
same('doc-low', $lowLevel->update(['id' => 'doc-low', 'operations' => []])['id'], 'low-level update');
same(true, $lowLevel->delete(['id' => 'doc-low'])['deleted'], 'low-level delete');
same(null, $lowLevel->get(['id' => 'doc-low'])['document'], 'low-level get');
same('Exact', $lowLevel->search(['query' => ['source_type' => 'MatchAllQuery', 'source' => []]])['total_hits']['relation'], 'low-level search');
same('Running', $lowLevel->analyze(['text' => 'Running'])['tokens'][0]['surface'], 'low-level analyze');
same('test', $lowLevel->describe()['index']['name'], 'low-level describe');
same(
    'MatchAllQuery',
    $lowLevel->inspectQuery(['request' => ['query' => ['source_type' => 'MatchAllQuery', 'source' => []]]])['resolved_request']['query']['source_type'],
    'low-level inspect query',
);

$schema = new SearchKernel\IndexDefinition(
    fields: [
        'title' => SearchKernel\Field::text(stored: true, searchable: true),
        'people' => SearchKernel\Field::arrayOf(
            SearchKernel\Field::structOf([
                'first' => SearchKernel\Field::keyword(),
                'last' => SearchKernel\Field::keyword(),
            ]),
            stored: false,
            searchable: false,
        ),
        'location' => SearchKernel\Field::position(stored: false),
    ],
    rankProfiles: [
        (new SearchKernel\RankProfile('semantic'))
            ->firstPhase('closeness(embedding)')
            ->queryInput('query_embedding', 'tensor<float>(x[8])'),
    ],
);
$schemaWire = $schema->toWire();
same(['location', 'people', 'title'], array_column($schemaWire['fields'], 'name'), 'index definition fields are canonical key order');
$peopleField = current(array_values(array_filter(
    $schemaWire['fields'],
    static fn (array $field): bool => $field['name'] === 'people',
)));
truth(is_array($peopleField), 'people field present in index definition');
same('ArrayDataType', $peopleField['data_type']['kind_type'], 'people field keeps array type');
same(
    'StructDataType',
    $peopleField['data_type']['kind']['element_type']['kind_type'],
    'people array keeps struct element type',
);
$locationField = current(array_values(array_filter(
    $schemaWire['fields'],
    static fn (array $field): bool => $field['name'] === 'location',
)));
truth(is_array($locationField), 'location field present in index definition');
same('PositionDataType', $locationField['data_type']['kind_type'], 'location field keeps position type');

require_once dirname(__DIR__) . '/examples/searchkernel_client_api_e2e_schema.php';
$parityCorpus = json_decode(
    (string) file_get_contents(dirname(__DIR__, 3) . '/reference/client-real-e2e-parity/corpus.json'),
    true,
    flags: JSON_THROW_ON_ERROR,
);
$typedCorpusDocument = searchkernelPhpClientApiE2eDocument($parityCorpus, [
    '_id' => 'typed-corpus-document',
    'price' => 89,
    'sale_price' => 79,
    'rating' => 5,
]);
truth(is_float($typedCorpusDocument['price']), 'E2E corpus integer for Float64 field becomes float');
truth(is_float($typedCorpusDocument['sale_price']), 'E2E corpus optional Float64 field becomes float');
same(5, $typedCorpusDocument['rating'], 'E2E corpus integer field remains integer');
same(50.0, searchkernelPhpClientApiE2eQueryValue($parityCorpus, 'price', 50), 'E2E range bound matches the declared Float64 field');
same(5, searchkernelPhpClientApiE2eQueryValue($parityCorpus, 'rating', 5), 'E2E query value preserves declared Int64 field');
$parityDefinition = searchkernelPhpClientApiE2eDefinition($parityCorpus)->toWire();
$parityFieldNames = array_column($parityDefinition['fields'], 'name');
$expectedParityFieldNames = array_keys($parityCorpus['schema']['fields']);
sort($expectedParityFieldNames, SORT_STRING);
same($expectedParityFieldNames, $parityFieldNames, 'PHP E2E schema is built only from shared corpus fields');
$parityPeople = current(array_values(array_filter(
    $parityDefinition['fields'],
    static fn (array $field): bool => $field['name'] === 'people',
)));
same('ArrayDataType', $parityPeople['data_type']['kind_type'], 'PHP parity people type follows corpus');
same(
    ['first', 'last'],
    array_column($parityPeople['data_type']['kind']['element_type']['kind']['fields'], 'name'),
    'PHP parity people members follow corpus',
);
$parityLocation = current(array_values(array_filter(
    $parityDefinition['fields'],
    static fn (array $field): bool => $field['name'] === 'location',
)));
same('PositionDataType', $parityLocation['data_type']['kind_type'], 'PHP parity location type follows corpus');
same(
    array_column($parityCorpus['schema']['rank_profiles'], 'name'),
    array_column($parityDefinition['rank_profiles'], 'name'),
    'PHP E2E rank profiles follow the shared corpus',
);
foreach ($parityCorpus['schema']['rank_profiles'] as $position => $spec) {
    $actualProfile = $parityDefinition['rank_profiles'][$position];
    foreach (['first_phase', 'second_phase'] as $phase) {
        if (isset($spec[$phase])) {
            same($spec[$phase], $actualProfile[$phase]['expression'], 'PHP rank profile phase follows corpus: ' . $spec['name'] . '.' . $phase);
        }
    }
    same($spec['match_features'] ?? [], $actualProfile['match_features'] ?? [], 'PHP rank profile match features follow corpus: ' . $spec['name']);
    if (isset($spec['rerank_count'])) {
        same($spec['rerank_count'], $actualProfile['second_phase']['rerank_count']['value'], 'PHP rerank count follows corpus: ' . $spec['name']);
    }
    if (isset($spec['query_inputs'])) {
        same(array_keys($spec['query_inputs']), array_column($actualProfile['inputs'], 'name'), 'PHP rank profile inputs follow corpus: ' . $spec['name']);
    }
}
$parityEmbedding = current(array_values(array_filter(
    $parityDefinition['fields'],
    static fn (array $field): bool => $field['name'] === 'embedding',
)));
same(
    $parityCorpus['schema']['fields']['embedding']['type'],
    $parityEmbedding['data_type']['kind']['type_spec'],
    'PHP E2E tensor schema follows the shared corpus type',
);
foreach ($parityCorpus['schema']['fields'] as $name => $spec) {
    $actual = current(array_values(array_filter(
        $parityDefinition['fields'],
        static fn (array $field): bool => $field['name'] === $name,
    )));
    $expectedType = match (true) {
        $spec['type'] === 'text', $spec['type'] === 'keyword' => [
            'kind_type' => 'PrimitiveDataType', 'kind' => ['kind' => 'String'],
        ],
        $spec['type'] === 'float64' => ['kind_type' => 'PrimitiveDataType', 'kind' => ['kind' => 'Float64']],
        $spec['type'] === 'int64' => ['kind_type' => 'PrimitiveDataType', 'kind' => ['kind' => 'Int64']],
        $spec['type'] === 'bool' => ['kind_type' => 'PrimitiveDataType', 'kind' => ['kind' => 'Bool']],
        $spec['type'] === 'position' => ['kind_type' => 'PositionDataType'],
        $spec['type'] === 'array' => ['kind_type' => 'ArrayDataType'],
        str_starts_with($spec['type'], 'tensor<') => ['kind_type' => 'TensorDataType', 'kind' => ['type_spec' => $spec['type']]],
        default => throw new RuntimeException('unhandled shared corpus field type'),
    };
    if (array_key_exists('kind', $expectedType)) {
        same($expectedType, $actual['data_type'], 'PHP field data type follows shared corpus: ' . $name);
    } else {
        same($expectedType['kind_type'], $actual['data_type']['kind_type'], 'PHP field data type follows shared corpus: ' . $name);
    }
    foreach (['stored', 'searchable', 'filterable', 'sortable', 'facetable'] as $capability) {
        if (array_key_exists($capability, $spec)) {
            same(['value' => $spec[$capability]], $actual[$capability] ?? null, 'PHP field capability follows shared corpus: ' . $name . '.' . $capability);
        }
    }
}

$lifecycleTransport = new RecordingTransport();
$lifecycle = new Client($lifecycleTransport, 'products');
same(
    $schemaWire,
    $lifecycle->createIndex(['definition' => $schemaWire])['index'],
    'create-index response preserves canonical definition',
);
same(['name' => 'products'], $lifecycleTransport->requests[0]['target'], 'create-index carries logical target');
same([], $lifecycle->openIndex()['index']['fields'], 'open-index result accepted');
$openRequest = $lifecycleTransport->requests[array_key_last($lifecycleTransport->requests)];
truth(
    $openRequest['payload'] instanceof SearchKernel\JsonObject,
    'open-index uses explicit empty-table payload',
);
truth(
    str_contains(Protocol::encodeJson($openRequest), '"payload":{}'),
    'open-index JSON emits an empty object payload',
);
$lifecycle->deleteIndex();
$deleteRequest = $lifecycleTransport->requests[array_key_last($lifecycleTransport->requests)];
truth(
    $deleteRequest['payload'] instanceof SearchKernel\JsonObject,
    'delete-index uses explicit empty-table payload',
);
truth(
    str_contains(Protocol::encodeJson($deleteRequest), '"payload":{}'),
    'delete-index JSON emits an empty object payload',
);
foreach ($lifecycleTransport->requests as $request) {
    same(['name' => 'products'], $request['target'], 'target is carried on every lifecycle request');
}

$unsafeTargetTransport = new RecordingTransport();
(new Client($unsafeTargetTransport, '../escape'))->openIndex();
same(
    ['name' => '../escape'],
    $unsafeTargetTransport->requests[0]['target'],
    'domain-invalid target reaches canonical semantic validation',
);

throws(
    InvalidArgumentException::class,
    static fn () => Protocol::operation('future_operation'),
    'unknown low-level operation fails closed',
);

// --------------------------------------------------------------------------
// Final strict-decoding regressions: canonical JSON container/enum fidelity.
// These live after SKC annotations so adding them does not churn evidence lines.
// --------------------------------------------------------------------------

throws(
    ProtocolException::class,
    static fn () => Protocol::decodeValue([
        'kind_type' => 'ValueList',
        'kind' => ['values' => null],
    ]),
    'present-null ValueList vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchKernel\Hit::fromJson([
        'id' => 'doc',
        'fields' => null,
    ]),
    'present-null Hit fields vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchResult::fromPayload(canonicalResultPayload(['errors' => null])),
    'present-null SearchResult errors vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchKernel\ResultGroup::decodeNode([
        'node_type' => 'ResultGroup',
        'node' => ['children' => null],
    ]),
    'present-null ResultGroup children vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => new SearchKernel\Coverage(['metadata' => null]),
    'present-null Coverage metadata vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => new SearchKernel\SearchTiming(['metadata' => null]),
    'present-null SearchTiming metadata vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => new SearchKernel\ResultIssue(['metadata' => null]),
    'present-null result-error metadata vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchKernelException::fromPayload([
        'code' => 'InvalidArgument',
        'message' => 'bad',
        'retryable' => false,
        'details' => null,
    ]),
    'present-null canonical Error details vector fails closed',
);
throws(
    ProtocolException::class,
    static fn () => SearchKernelException::fromPayload([
        'code' => 'InvalidArgument',
        'message' => 'bad',
        'retryable' => false,
        'details' => [[
            'code' => 'bad_detail',
            'message' => 'bad',
            'metadata' => [
                'not-a-vector' => ['key' => 'k', 'value' => 'v'],
            ],
        ]],
    ]),
    'canonical Error detail metadata must be a vector',
);
throws(
    ProtocolException::class,
    static fn () => SearchKernelException::fromPayload([
        'code' => 'FutureErrorCode',
        'message' => 'bad',
        'retryable' => false,
        'details' => [],
    ]),
    'unknown canonical Error code fails closed',
);
same(
    'Unspecified',
    SearchResult::fromPayload(canonicalResultPayload([
        'total_hits' => ['value' => 0],
    ]))->totalHitsRelation,
    'present TotalHits with omitted relation preserves the schema default',
);
throws(
    ProtocolException::class,
    static fn () => SearchResult::fromPayload(canonicalResultPayload([
        'total_hits' => ['value' => 0, 'relation' => 'FutureRelation'],
    ])),
    'unknown TotalHits relation fails closed',
);

// --------------------------------------------------------------------------
// Public-constructor branch coverage.
// --------------------------------------------------------------------------
// These assertions intentionally exercise sibling helpers that share lowering
// code but carry branch-specific operators, union tags, or presence semantics.

$fromArrayObject = ValueObject::fromArray(['z' => 2, 'a' => 1]);
same(
    ['a', 'z'],
    array_column($fromArrayObject->entries, 'key'),
    'ValueObject::fromArray canonicalizes object key order',
);
throws(
    \InvalidArgumentException::class,
    static fn () => ValueObject::fromArray([0 => 'numeric-key']),
    'ValueObject::fromArray rejects PHP integer keys rather than changing portable object keys',
);

$mixedTensor = Tensor::mixed('tensor<float>(a{},z{},x[2])', [[
    'address' => ['z' => 'last', 'a' => 'first'],
    'values' => [1, 2.5],
]]);
$mixedTensorWire = Protocol::value($mixedTensor);
same('TensorValue', $mixedTensorWire['kind_type'], 'mixed tensor uses TensorValue union');
same('MixedTensorBlocks', $mixedTensorWire['kind']['payload_type'], 'mixed tensor payload discriminator');
same(
    ['a', 'z'],
    array_column($mixedTensorWire['kind']['payload']['blocks'][0]['address'], 'dimension'),
    'mixed tensor mapped address is canonical dimension order',
);
same(
    [1.0, 2.5],
    Tensor::fromWire(
        'tensor<float>(m{},x[2])',
        'MixedTensorBlocks',
        ['blocks' => [[
            'address' => [['dimension' => 'm', 'label' => 'key']],
            'values' => [1, 2.5],
        ]]],
    )->payload['blocks'][0]['values'],
    'mixed tensor wire decoder validates and preserves block values',
);

foreach ([
    'ne' => 'Ne',
    'lt' => 'Lt',
    'lte' => 'Lte',
    'gt' => 'Gt',
    'gte' => 'Gte',
] as $comparisonMethod => $comparisonOperator) {
    $comparison = SearchKernel\Query::$comparisonMethod('price', 10);
    same('QueryComparison', $queryExpr($comparison)['expr_type'], $comparisonMethod . ' comparison type');
    same(
        $comparisonOperator,
        $queryExpr($comparison)['expr']['op'],
        $comparisonMethod . ' comparison operator',
    );
}
$queryOr = SearchKernel\Query::anyOf(
    SearchKernel\Query::alwaysTrue(),
    SearchKernel\Query::alwaysFalse(),
);
same('QueryOr', $queryExpr($queryOr)['expr_type'], 'boolean OR query');
same(
    'QueryTrue',
    $queryExpr($queryOr)['expr']['clauses'][0]['expr_type'],
    'alwaysTrue query branch',
);

$constructorSearch = $index
    ->search('fox')
    ->searchFields('title', 'body')
    ->sortFeature('nativeRank', 'asc')
    ->rawSort('price * -1', 'desc')
    ->offset(0)
    ->limit(5)
    ->request();
same(
    ['title', 'body'],
    array_column($constructorSearch['search_fields']['fields'], 'field'),
    'searchFields convenience preserves caller order',
);
same(
    'RankFeatureSortTarget',
    $constructorSearch['sort'][0]['target']['target_type'],
    'rank-feature sort target discriminator',
);
same('Asc', $constructorSearch['sort'][0]['direction'], 'rank-feature sort direction');
same(
    'RawSortTarget',
    $constructorSearch['sort'][1]['target']['target_type'],
    'raw sort target discriminator',
);
same('Desc', $constructorSearch['sort'][1]['direction'], 'raw sort direction');
same(['value' => 0], $constructorSearch['offset'], 'explicit zero offset remains present');
same(['value' => 5], $constructorSearch['limit'], 'explicit limit remains present');

$yqlOffset = $index
    ->yql('select * from sources * where true')
    ->offset(0)
    ->limit(5)
    ->request();
same(['value' => 0], $yqlOffset['offset'], 'YQL explicit zero offset remains present');
same(['value' => 5], $yqlOffset['limit'], 'YQL explicit limit remains present');

$index->analyze('Running')
    ->analyzer('custom', 7, ['mode' => 'strict'])
    ->language('en')
    ->execute();
$analyzeWithAnalyzer = $transport->requests[array_key_last($transport->requests)];
same(
    'Analyzer',
    $analyzeWithAnalyzer['payload']['target']['target_type'],
    'analyze explicit analyzer target discriminator',
);
same(
    7,
    $analyzeWithAnalyzer['payload']['target']['target']['version'],
    'analyze explicit analyzer version',
);
same(
    'mode',
    $analyzeWithAnalyzer['payload']['target']['target']['options'][0]['key'],
    'analyze options use canonical value-entry encoding',
);
same('en', $analyzeWithAnalyzer['payload']['language'], 'analyze language preserved');

$branchUpdateId = $index->update('doc-branch')
    ->decrement('inventory', 2)
    ->divide('price', 4)
    ->execute();
same('doc-branch', $branchUpdateId, 'branch arithmetic update result id');
$branchUpdate = $transport->requests[array_key_last($transport->requests)];
same(
    'Decrement',
    $branchUpdate['payload']['operations'][0]['operation']['operator'],
    'update decrement operator',
);
same(
    'Divide',
    $branchUpdate['payload']['operations'][1]['operation']['operator'],
    'update divide operator',
);
same(
    'Decrement',
    SearchKernel\MatchedUpdate::decrement(3)->toWire()['operation']['operator'],
    'matched update decrement operator',
);
same(
    'Divide',
    SearchKernel\MatchedUpdate::divide(3)->toWire()['operation']['operator'],
    'matched update divide operator',
);

$groupField = SearchKernel\G::field('price');
foreach ([
    [$groupField->plus(1), 'Add'],
    [$groupField->minus(1), 'Subtract'],
    [$groupField->divide(2), 'Divide'],
    [$groupField->modulo(3), 'Modulo'],
] as [$groupExpression, $groupOperator]) {
    same('GroupingBinaryExpression', $groupExpression->model['expr_type'], $groupOperator . ' grouping expression type');
    same($groupOperator, $groupExpression->model['expr']['op'], $groupOperator . ' grouping operator');
}
same(
    'Negative',
    $groupField->negate()->model['expr']['op'],
    'grouping negate operator',
);
same(
    'Positive',
    SearchKernel\GroupingOrder::asc($groupField)->expression->model['expr']['op'],
    'ascending grouping order uses positive unary expression',
);
same(
    'GroupingMemberExpression',
    SearchKernel\G::member(SearchKernel\G::field('profile'), 'country')->model['expr_type'],
    'grouping member expression discriminator',
);
same(
    'GroupingMapLookupExpression',
    SearchKernel\G::lookup(SearchKernel\G::field('attributes'), 'color')->model['expr_type'],
    'grouping map lookup expression discriminator',
);
$bucket = SearchKernel\G::bucket(10, 20, false, true)->model;
same('GroupingBucketExpression', $bucket['expr_type'], 'grouping bucket expression discriminator');
same(['value' => false], $bucket['expr']['include_lower'], 'bucket lower inclusion presence');
same(['value' => true], $bucket['expr']['include_upper'], 'bucket upper inclusion presence');

$isTrue = SearchKernel\Gf::isTrue(SearchKernel\G::field('in_stock'));
$filterOr = SearchKernel\Gf::anyOf(
    $isTrue,
    SearchKernel\Gf::regex(SearchKernel\G::field('brand'), '^A'),
);
same('GroupingFilterOr', $filterOr['expr_type'], 'grouping filter OR discriminator');
same('GroupingFilterIsTrue', $filterOr['expr']['clauses'][0]['expr_type'], 'grouping is-true filter');

foreach ([
    [SearchKernel\Agg::min('price'), 'MinAggregation'],
    [SearchKernel\Agg::max('price'), 'MaxAggregation'],
    [SearchKernel\Agg::stddev('price'), 'StdDevAggregation'],
] as [$aggregation, $aggregationType]) {
    same(
        $aggregationType,
        $aggregation->model['expr']['aggregation_type'],
        $aggregationType . ' discriminator',
    );
}

$nestedGroup = SearchKernel\Group::by('brand')
    ->subgroup(SearchKernel\Group::by('category'))
    ->toRequest();
$nestedGroupBody = $nestedGroup['source']['root']['operations'][1]['operation']['operations'];
same(
    'GroupingAllOperation',
    $nestedGroupBody[0]['operation_type'],
    'subgroup lowers through GroupingAllOperation',
);

echo "PHP client semantic-spine tests PASS\n";
