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

use SearchKernel\Protocol;
use SearchKernel\SearchKernelException;
use SearchKernel\SearchResult;

function same(mixed $expected, mixed $actual, string $message): void
{
    if ($expected !== $actual) {
        throw new RuntimeException($message . "\nexpected: " . var_export($expected, true) . "\nactual: " . var_export($actual, true));
    }
}

$document = Protocol::document([
    '_id' => 'doc-1',
    'title' => 'The quick brown fox',
    'count' => 42,
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
same(42, $decoded['count'], 'decode int');
same(['one', 'two'], $decoded['tags'], 'decode list');
same(['a' => 1, 'z' => 2], $decoded['meta'], 'decode object');

$envelope = Protocol::envelope('SearchRequest', [
    'query' => [
        'source_type' => 'SearchKernelTextQuery',
        'source' => ['text' => 'brown fox'],
    ],
]);
same(2, $envelope['protocol_version'], 'protocol version');
same('SearchKernel_V2_SearchRequest', $envelope['payload_type'], 'request discriminator');

$result = SearchResult::fromPayload([
    'root' => [
        'node_type' => 'ResultGroup',
        'node' => [
            'children' => [
                [
                    'node_type' => 'Hit',
                    'node' => [
                        'id' => 'doc-1',
                        'fields' => [
                            [
                                'key' => 'title',
                                'value' => [
                                    'kind_type' => 'StringValue',
                                    'kind' => ['value' => 'The quick brown fox'],
                                ],
                            ],
                        ],
                    ],
                ],
            ],
        ],
    ],
    'total_hits' => ['value' => 1, 'relation' => 'Exact'],
]);
same(1, count($result->hits), 'search hit count');
same('doc-1', $result->hits[0]->id, 'search hit id');
same('The quick brown fox', $result->hits[0]->get('title'), 'search hit field');
same(1, $result->totalHits, 'total hits');
same('Exact', $result->totalHitsRelation, 'total hits relation');

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

try {
    Protocol::value(INF);
    throw new RuntimeException('non-finite input did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}



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
];
same(array_keys($expectedOperations), Protocol::operations(), 'portable operation list');
foreach ($expectedOperations as $operation => $types) {
    same($types, Protocol::operation($operation), 'operation mapping: ' . $operation);
}

$transport = new class($expectedOperations) implements \SearchKernel\Transport {
    /** @param array<string, array{0:string,1:string}> $operations */
    public function __construct(private readonly array $operations) {}

    public function execute(array $request): array
    {
        same(2, $request['protocol_version'] ?? null, 'low-level request protocol version');
        $requestType = $request['payload_type'] ?? null;
        foreach ($this->operations as [$expectedRequest, $expectedResponse]) {
            if ($requestType === 'SearchKernel_V2_' . $expectedRequest) {
                return [
                    'protocol_version' => 2,
                    'payload_type' => 'SearchKernel_V2_' . $expectedResponse,
                    'payload' => ['echo' => $request['payload'] ?? null],
                ];
            }
        }
        throw new RuntimeException('unexpected request type: ' . var_export($requestType, true));
    }
};

$client = new \SearchKernel\Client($transport);
foreach ($expectedOperations as $operation => $_types) {
    $payload = ['operation' => $operation];
    $result = $client->execute($operation, $payload);
    same($payload, $result['echo'] ?? null, 'low-level execute round trip: ' . $operation);
}

try {
    $client->execute('not_an_operation', []);
    throw new RuntimeException('unknown operation did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}

try {
    Protocol::expectPayload([
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_GetResult',
        'payload' => [],
    ], 'SearchResult');
    throw new RuntimeException('wrong response discriminator did not throw');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}

try {
    Protocol::expectPayload([
        'protocol_version' => 999,
        'payload_type' => 'SearchKernel_V2_SearchResult',
        'payload' => [],
    ], 'SearchResult');
    throw new RuntimeException('wrong protocol version did not throw');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}

try {
    Protocol::expectPayload([
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_Error',
        'payload' => [
            'code' => 'InvalidArgument',
            'message' => 'bad request',
            'retryable' => true,
            'details' => [
                [
                    'code' => 'unknown_field',
                    'path' => 'filters[0]',
                    'message' => 'unknown field',
                    'metadata' => [],
                ],
            ],
        ],
    ], 'SearchResult');
    throw new RuntimeException('portable error did not throw');
} catch (SearchKernelException $error) {
    same('InvalidArgument', $error->errorCode, 'portable error preserves code');
    same('bad request', $error->getMessage(), 'portable error preserves message');
    same(true, $error->retryable, 'portable error preserves retryable');
    same('filters[0]', $error->details[0]['path'] ?? null, 'portable error preserves detail path');
}

try {
    Protocol::expectPayload([
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_Error',
        'payload' => [
            'code' => 'InvalidArgument',
            'message' => 'default retryable',
        ],
    ], 'SearchResult');
    throw new RuntimeException('portable error did not throw');
} catch (SearchKernelException $error) {
    same(false, $error->retryable, 'omitted Error.retryable uses FlatBuffers false default');
}

try {
    Protocol::expectPayload([
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_Error',
        'payload' => ['message' => 'missing code'],
    ], 'SearchResult');
    throw new RuntimeException('malformed portable error did not fail closed');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}


same(false, Protocol::decodeValue([
    'kind_type' => 'BoolValue',
    'kind' => [],
]), 'omitted BoolValue scalar uses FlatBuffers false default');
same(0, Protocol::decodeValue([
    'kind_type' => 'Int64Value',
    'kind' => [],
]), 'omitted Int64Value scalar uses FlatBuffers zero default');
same('0', (string) Protocol::decodeValue([
    'kind_type' => 'UInt64Value',
    'kind' => [],
]), 'omitted UInt64Value scalar uses FlatBuffers zero default');
same(0.0, Protocol::decodeValue([
    'kind_type' => 'Float64Value',
    'kind' => [],
]), 'omitted Float64Value scalar uses FlatBuffers zero default');

$int64Max = new \SearchKernel\Int64('9223372036854775807');
$int64Min = new \SearchKernel\Int64('-9223372036854775808');
same('9223372036854775807', Protocol::value($int64Max)['kind']['value'], 'explicit int64 max stays decimal text');
same('-9223372036854775808', Protocol::value($int64Min)['kind']['value'], 'explicit int64 min stays decimal text');
same('Int64Value', Protocol::value($int64Max)['kind_type'], 'explicit int64 discriminator');
$decodedInt64 = Protocol::decodeValue([
    'kind_type' => 'Int64Value',
    'kind' => ['value' => '9223372036854775807'],
]);
same('9223372036854775807', (string) $decodedInt64, 'quoted int64 response remains lossless');

$uint64Max = new \SearchKernel\UInt64('18446744073709551615');
$encodedUInt64 = Protocol::value($uint64Max);
same('UInt64Value', $encodedUInt64['kind_type'], 'uint64 discriminator');
same('18446744073709551615', $encodedUInt64['kind']['value'], 'uint64 max stays decimal text');
$decodedUInt64 = Protocol::decodeValue($encodedUInt64);
same('18446744073709551615', (string) $decodedUInt64, 'uint64 max round trip');

try {
    new \SearchKernel\Int64('9223372036854775808');
    throw new RuntimeException('int64 overflow did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}
try {
    new \SearchKernel\Int64('-9223372036854775809');
    throw new RuntimeException('int64 underflow did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}
try {
    new \SearchKernel\UInt64('18446744073709551616');
    throw new RuntimeException('uint64 overflow did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}
try {
    new \SearchKernel\UInt64('01');
    throw new RuntimeException('non-canonical uint64 did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}

$binary = new \SearchKernel\Bytes("\x00\x01\x7f\x80\xff");
$encodedBytes = Protocol::value($binary);
same('BytesValue', $encodedBytes['kind_type'], 'bytes discriminator');
same([0, 1, 127, 128, 255], $encodedBytes['kind']['value'], 'bytes encode as octets');
$decodedBytes = Protocol::decodeValue($encodedBytes);
same("\x00\x01\x7f\x80\xff", $decodedBytes->value, 'bytes round trip as binary string');

foreach ([
    \SearchKernel\NonFiniteFloat::nan(),
    \SearchKernel\NonFiniteFloat::positiveInfinity(),
    \SearchKernel\NonFiniteFloat::negativeInfinity(),
] as $special) {
    $encoded = Protocol::value($special);
    same('NonFiniteFloatValue', $encoded['kind_type'], 'non-finite discriminator');
    same($special->kind, $encoded['kind']['kind'], 'non-finite kind encode');
    $decodedSpecial = Protocol::decodeValue($encoded);
    same($special->kind, $decodedSpecial->kind, 'non-finite round trip');
}

$unicodeEntries = Protocol::valueEntries([
    "\u{1F600}" => 4,
    "\u{10000}" => 3,
    "\u{E000}" => 2,
    'a' => 1,
]);
same(['a', "\u{E000}", "\u{10000}", "\u{1F600}"], array_column($unicodeEntries, 'key'), 'semantic maps use stable UTF-8/codepoint ordering');

try {
    Protocol::decodeValueEntries([
        ['key' => 'x', 'value' => Protocol::value(1)],
        ['key' => 'x', 'value' => Protocol::value(2)],
    ]);
    throw new RuntimeException('duplicate portable object key did not throw');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}

$dense = \SearchKernel\Tensor::dense('tensor<float>(x[3])', [1, 2.5, -3]);
$denseValue = Protocol::value($dense);
same('TensorValue', $denseValue['kind_type'], 'dense tensor discriminator');
same('DenseTensorValues', $denseValue['kind']['payload_type'], 'dense tensor payload discriminator');
same($dense->toJson(), Protocol::decodeValue($denseValue)->toJson(), 'dense tensor round trip');

$sparse = \SearchKernel\Tensor::sparse('tensor<float>(token{})', [
    ['address' => ['token' => 'fox'], 'value' => 1.25],
    ['address' => ['token' => 'dog'], 'value' => -0.5],
]);
$sparseValue = Protocol::value($sparse);
same($sparse->toJson(), Protocol::decodeValue($sparseValue)->toJson(), 'sparse tensor round trip');

$mixed = \SearchKernel\Tensor::mixed('tensor<float>(paragraph{},x[2])', [
    ['address' => ['paragraph' => 'p1'], 'values' => [1.0, 2.0]],
    ['address' => ['paragraph' => 'p2'], 'values' => [3.0, 4.0]],
]);
$mixedValue = Protocol::value($mixed);
same($mixed->toJson(), Protocol::decodeValue($mixedValue)->toJson(), 'mixed tensor round trip');

try {
    Protocol::decodeValue([
        'kind_type' => 'FutureValueKind',
        'kind' => [],
    ]);
    throw new RuntimeException('unknown future Value discriminator did not fail closed');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}

try {
    Protocol::decodeValue([
        'kind_type' => 'TensorValue',
        'kind' => [
            'type_spec' => 'tensor<float>(x[1])',
            'payload_type' => 'FutureTensorPayload',
            'payload' => [],
        ],
    ]);
    throw new RuntimeException('unknown tensor payload discriminator did not fail closed');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}

try {
    Protocol::decodeValue([
        'kind_type' => 'BytesValue',
        'kind' => ['value' => [256]],
    ]);
    throw new RuntimeException('invalid byte did not fail closed');
} catch (\SearchKernel\ProtocolException) {
    // Expected.
}

try {
    \SearchKernel\Tensor::dense('tensor<float>(x[1])', [INF]);
    throw new RuntimeException('non-finite tensor cell did not throw');
} catch (InvalidArgumentException) {
    // Expected.
}

echo "PHP portable-value tests PASS\n";
