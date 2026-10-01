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
use SearchKernel\Group;
use SearchKernel\Index;
use SearchKernel\JsonObject;
use SearchKernel\Protocol;
use SearchKernel\Query;
use SearchKernel\Tensor;
use SearchKernel\Transport;
use SearchKernel\UInt64;
use SearchKernel\ValueObject;

require dirname(__DIR__) . '/examples/searchkernel_client_api_e2e_schema.php';

/**
 * Emit one actual wire JSON document. The Rust contract test feeds these bytes
 * through the official BFBS-backed FlatBuffers JSON bridge.
 *
 * @param array<string,mixed> $value
 */
function emitCase(string $schema, string $name, array $value): void
{
    echo $schema, "\t", $name, "\t", Protocol::encodeJson($value), PHP_EOL;
}

final class CaptureTransport implements Transport
{
    /** @var list<array<string,mixed>> */
    public array $requests = [];

    /** @param array<string,mixed> $request
     *  @return array<string,mixed>
     */
    public function execute(array $request): array
    {
        $this->requests[] = $request;
        $type = $request['payload_type'] ?? null;
        $payload = is_array($request['payload'] ?? null) ? $request['payload'] : [];

        [$resultType, $resultPayload] = match ($type) {
            'SearchKernel_Wire_PatchRequest' => ['PatchResult', ['id' => $payload['id'] ?? '']],
            'SearchKernel_Wire_UpdateRequest' => ['UpdateResult', ['id' => $payload['id'] ?? '']],
            default => throw new RuntimeException('unexpected capture request: ' . var_export($type, true)),
        };

        return Protocol::envelope($resultType, $resultPayload);
    }
}

$document = Protocol::document([
    '_id' => 'bfbs-doc',
    'title' => 'The quick brown fox',
    'count' => PHP_INT_MAX,
    'unsigned_max' => UInt64::fromDecimal('18446744073709551615'),
    'raw' => new Bytes("\x00\xffA"),
    'empty_object' => ValueObject::fromPairs([]),
    'tags' => ['php', 'bfbs'],
    'embedding' => Tensor::dense('tensor<float>(x[2])', [1.0, 2.0]),
]);
emitCase('request', 'insert-rich-values', Protocol::envelope('InsertRequest', [
    'document' => $document,
]));

$capture = new CaptureTransport();
$index = new Index($capture);

$index->patch('bfbs-doc')
    ->set('price', 12.5)
    ->unset('obsolete')
    ->execute();
emitCase('request', 'patch-unions', $capture->requests[array_key_last($capture->requests)]);

$index->update('bfbs-doc')
    ->assign('price', 20.0)
    ->increment('inventory', 2)
    ->tensorModify(
        'embedding',
        'replace',
        Tensor::sparse('tensor<float>(x{})', [[
            'address' => ['x' => 'alice'],
            'value' => 0.75,
        ]]),
        false,
    )
    ->condition('version == 7')
    ->create(false)
    ->execute();
emitCase('request', 'update-unions', $capture->requests[array_key_last($capture->requests)]);

$search = $index
    ->query(Query::allOf(
        Query::contains('title', 'fox'),
        Query::nearestNeighbor('embedding', 'query_embedding'),
    ))
    ->filter('price <= :max', ['max' => 120])
    ->sort('price', 'desc', 'last')
    ->fields('title', 'price')
    ->ranking('default')
    ->input('query_embedding', Tensor::dense('tensor<float>(x[2])', [0.25, 0.5]))
    ->group(Group::by('brand')->limit(5)->output(Agg::count()))
    ->aggregate(Agg::sum('price'))
    ->facet('category', 'category', 7, 'count_desc', 11)
    ->limit(10)
    ->request();
emitCase('request', 'search-query-grouping-ranking', Protocol::envelope('SearchRequest', $search));

$yql = $index
    ->yql('select * from sources * where title contains text(@q)', ['q' => 'fox'])
    ->ranking('default')
    ->limit(3)
    ->request();
emitCase('request', 'search-yql', Protocol::envelope('SearchRequest', $yql));

emitCase('request', 'describe', Protocol::envelope('DescribeRequest', []));
emitCase('request', 'inspect-query', Protocol::envelope('InspectQueryRequest', [
    'request' => $search,
]));

$createDefinition = searchkernelPhpClientApiE2eDefinition(searchkernelPhpClientApiE2eCorpus());
emitCase(
    'request',
    'create-index-e2e-schema',
    Protocol::envelope(
        'CreateIndexRequest',
        ['definition' => $createDefinition->toWire()],
        'php-e2e',
    ),
);

emitCase(
    'request',
    'open-index-empty-table',
    Protocol::envelope(
        'OpenIndexRequest',
        JsonObject::empty(),
        'php-e2e',
    ),
);

emitCase('response', 'portable-error', Protocol::envelope('Error', [
    'code' => 'InvalidArgument',
    'message' => 'bad request',
    'retryable' => false,
    'details' => [[
        'code' => 'unknown_field',
        'path' => 'query',
        'message' => 'field is unknown',
        'metadata' => [],
    ]],
]));

emitCase('response', 'recursive-search-result', Protocol::envelope('SearchResult', [
    'root' => [
        'node_type' => 'ResultGroup',
        'node' => [
            'label' => 'brand',
            'children' => [[
                'node_type' => 'Hit',
                'node' => [
                    'id' => 'bfbs-doc',
                    'fields' => [[
                        'key' => 'title',
                        'value' => Protocol::value('The quick brown fox'),
                    ]],
                    'relevance' => [
                        'value_type' => 'FiniteRelevance',
                        'value' => ['value' => 1.0],
                    ],
                ],
            ]],
        ],
    ],
    'total_hits' => ['value' => '1', 'relation' => 'Exact'],
]));

emitCase('response', 'default-delete-false', Protocol::envelope('DeleteResult', [
    'id' => 'bfbs-doc',
    'deleted' => false,
]));

emitCase('response', 'default-error-retryable-false', Protocol::envelope('Error', [
    'code' => 'InvalidArgument',
    'message' => 'not retryable',
    'retryable' => false,
    'details' => [],
]));

emitCase('response', 'default-analyze-zero', Protocol::envelope('AnalyzeResult', [
    'tokens' => [[
        'surface' => 'fox',
        'term' => 'fox',
        'position' => 0,
        'start_byte' => 0,
        'end_byte' => 3,
    ]],
]));

emitCase('response', 'default-highlight-zero', Protocol::envelope('SearchResult', [
    'total_hits' => ['value' => '1', 'relation' => 'Exact'],
    'root' => [
        'node_type' => 'Hit',
        'node' => [
            'id' => 'bfbs-doc',
            'highlights' => [[
                'field' => 'title',
                'fragments' => [[
                    'text' => 'fox',
                    'matches' => [[
                        'start_byte' => 0,
                        'end_byte' => 3,
                    ]],
                ]],
            ]],
        ],
    ],
]));

emitCase('response', 'default-relevance-zero', Protocol::envelope('SearchResult', [
    'total_hits' => ['value' => '1', 'relation' => 'Exact'],
    'root' => [
        'node_type' => 'Hit',
        'node' => [
            'id' => 'bfbs-doc',
            'relevance' => [
                'value_type' => 'FiniteRelevance',
                'value' => ['value' => 0.0],
            ],
        ],
    ],
]));

emitCase('response', 'default-sparse-tensor-zero', Protocol::envelope('GetResult', [
    'document' => Protocol::document([
        '_id' => 'bfbs-doc',
        'embedding' => Tensor::sparse(
            'tensor<float>(x{})',
            [[
                'address' => ['x' => 'alice'],
                'value' => 0.0,
            ]],
        ),
    ]),
]));


emitCase('request', 'insert-empty-string-bytes', Protocol::envelope('InsertRequest', [
    'document' => Protocol::document([
        '_id' => 'empty-values',
        'title' => '',
        'raw' => new Bytes(''),
    ]),
]));

emitCase('response', 'portable-zero-values', Protocol::envelope('SearchResult', [
    'total_hits' => ['value' => '1', 'relation' => 'Exact'],
    'root' => [
        'node_type' => 'Hit',
        'node' => [
            'id' => 'zero-values',
            'fields' => [
                ['key' => 'bool', 'value' => Protocol::value(false)],
                ['key' => 'float', 'value' => Protocol::value(0.0)],
                ['key' => 'int', 'value' => Protocol::value(0)],
                ['key' => 'uint', 'value' => Protocol::value(UInt64::fromDecimal('0'))],
            ],
        ],
    ],
]));
