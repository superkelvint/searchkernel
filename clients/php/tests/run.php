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
        'protocol_version' => 2,
        'payload_type' => 'SearchKernel_V2_SearchResult',
    ], 'SearchResult');
    throw new RuntimeException('missing response union payload did not throw');
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
    same(false, $error->retryable, 'omitted FlatBuffers bool uses schema default false');
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

echo "PHP portable-core tests PASS\\n";
