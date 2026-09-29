<?php
declare(strict_types=1);

spl_autoload_register(static function (string $class): void {
    $prefix = 'SearchKernel\\';
    if (str_starts_with($class, $prefix)) {
        $path = dirname(__DIR__) . '/src/' . substr($class, strlen($prefix)) . '.php';
        if (is_file($path)) require $path;
    }
});

use SearchKernel\Bytes;
use SearchKernel\Hit;
use SearchKernel\Protocol;
use SearchKernel\ResultGroup;
use SearchKernel\SearchResult;
use SearchKernel\Tensor;

function traceValue(mixed $value): mixed
{
    if ($value === null || is_bool($value) || is_string($value)) {
        return $value;
    }
    if (is_int($value)) {
        return (string) $value;
    }
    if (is_float($value)) {
        if (is_nan($value)) return 'nan';
        if ($value === INF) return '+inf';
        if ($value === -INF) return '-inf';
        return $value;
    }
    if ($value instanceof Bytes) {
        $bytes = unpack('C*', $value->value);
        return array_map(static fn (int $item): string => (string) $item, $bytes === false ? [] : array_values($bytes));
    }
    if ($value instanceof Tensor) {
        $tensor = ['type_spec' => $value->typeSpec];
        $tensor[match ($value->payloadType) {
            'DenseTensorValues' => 'dense',
            'SparseTensorCells' => 'sparse',
            'MixedTensorBlocks' => 'mixed',
            default => throw new RuntimeException('unknown tensor payload type in trace'),
        }] = traceValue(match ($value->payloadType) {
            'DenseTensorValues' => $value->payload['values'],
            'SparseTensorCells' => $value->payload['cells'],
            'MixedTensorBlocks' => $value->payload['blocks'],
        });
        return ['$searchkernel_tensor' => $tensor];
    }
    if (is_array($value)) {
        if (array_is_list($value)) {
            return array_map(traceValue(...), $value);
        }
        ksort($value, SORT_STRING);
        $normalized = [];
        foreach ($value as $key => $item) {
            $normalized[$key] = traceValue($item);
        }
        return $normalized;
    }
    throw new RuntimeException('unsupported trace value: ' . get_debug_type($value));
}

function traceNode(Hit|ResultGroup|null $node): mixed
{
    if ($node === null) return null;
    if ($node instanceof Hit) {
        return [
            'kind' => 'hit',
            'id' => $node->id,
            'relevance' => traceValue($node->relevance),
            'fields' => traceValue($node->fields),
            'highlights' => array_map(
                static fn (array $highlight): array => [
                    'field' => $highlight['field'],
                    'fragments' => array_map(
                        static fn (array $fragment): array => [
                            'text' => $fragment['text'],
                            'matches' => array_map(
                                static fn (array $match): array => [
                                    'start_byte' => (string) $match[0],
                                    'end_byte' => (string) $match[1],
                                ],
                                $fragment['matches'],
                            ),
                        ],
                        $highlight['fragments'],
                    ),
                ],
                $node->highlights,
            ),
            'source' => $node->source,
            'types' => $node->types,
            'match_features' => traceValue($node->matchFeatures),
            'summary_features' => traceValue($node->summaryFeatures),
            'rank_features' => traceValue($node->rankFeatures),
            'sort_values' => traceValue($node->sortValues),
        ];
    }
    $continuations = [];
    foreach ($node->continuations as $key => $value) {
        $continuations[] = ['key' => $key, 'value' => $value];
    }
    return [
        'kind' => 'group',
        'id' => $node->id,
        'label' => $node->label,
        'value' => traceValue($node->value),
        'fields' => traceValue($node->fields),
        'children' => array_map(traceNode(...), $node->children),
        'continuations' => $continuations,
        'source' => $node->source,
        'types' => $node->types,
        'relevance' => traceValue($node->relevance),
        'limits' => $node->limits === null ? null : [
            'from' => traceValue($node->limits['from']),
            'to' => traceValue($node->limits['to']),
        ],
        'metadata' => traceValue($node->metadata),
    ];
}

function traceRelation(string $relation): string
{
    return match ($relation) {
        'Unspecified' => 'unspecified',
        'Exact' => 'exact',
        'LowerBound' => 'lower_bound',
        'Approximate' => 'approximate',
        'Unknown' => 'unknown',
        default => throw new RuntimeException('unknown total-hit relation in trace: ' . $relation),
    };
}

function traceCoverage(mixed $coverage): mixed
{
    if ($coverage === null) return null;
    $degraded = $coverage->degraded;
    return [
        'coverage_percent' => traceValue($coverage->coveragePercent),
        'documents' => traceValue($coverage->documents),
        'full' => $coverage->full,
        'nodes' => traceValue($coverage->nodes),
        'results' => traceValue($coverage->results),
        'results_full' => traceValue($coverage->resultsFull),
        'degraded' => $degraded === null ? null : [
            'match_phase' => $degraded->matchPhase,
            'timeout' => $degraded->timeout,
            'adaptive_timeout' => $degraded->adaptiveTimeout,
            'ann_timeout' => $degraded->annTimeout,
            'non_ideal_state' => $degraded->nonIdealState,
            'metadata' => traceValue($degraded->metadata),
        ],
        'metadata' => traceValue($coverage->metadata),
    ];
}

function traceTiming(mixed $timing): mixed
{
    if ($timing === null) return null;
    return [
        'query_time_seconds' => traceValue($timing->queryTimeSeconds),
        'search_time_seconds' => traceValue($timing->searchTimeSeconds),
        'summary_fetch_time_seconds' => traceValue($timing->summaryFetchTimeSeconds),
        'metadata' => traceValue($timing->metadata),
    ];
}

function traceResult(SearchResult $result): array
{
    return [
        'root' => traceNode($result->root),
        'total_hits' => [
            'value' => (string) $result->totalHits,
            'relation' => traceRelation($result->totalHitsRelation),
        ],
        'coverage' => traceCoverage($result->coverage),
        'timing' => traceTiming($result->timing),
        'errors' => array_map(
            static fn ($error): array => [
                'code' => (string) $error->code,
                'summary' => $error->summary,
                'message' => $error->message,
                'source' => $error->source,
                'stack_trace' => $error->stackTrace,
                'transient' => $error->transient,
                'metadata' => traceValue($error->metadata),
            ],
            $result->errors,
        ),
        'trace' => traceValue($result->trace),
        'metadata' => traceValue($result->metadata),
    ];
}

$cases = json_decode(file_get_contents(getenv('SEARCHKERNEL_RESULT_CASES')), true, 512, JSON_THROW_ON_ERROR);
$caseDirectory = getenv('SEARCHKERNEL_RESULT_CASE_DIR');
$traceDirectory = getenv('SEARCHKERNEL_RESULT_TRACE_DIR');
$traces = [];
foreach ($cases['valid'] as $scenario) {
    $wire = json_decode(
        file_get_contents($caseDirectory . '/' . $scenario['json']),
        true,
        512,
        JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING,
    );
    $payload = Protocol::expectPayload($wire, 'SearchResult');
    $result = SearchResult::fromPayload($payload);
    if ($result->root === null) throw new RuntimeException('SearchResult root missing: ' . $scenario['id']);
    $traces[$scenario['id']] = traceResult($result);
}

$outcomes = [];
foreach ($cases['malformed'] as $scenario) {
    if ($scenario['bridge_rejected']) {
        $outcomes[$scenario['id']] = 'bridge';
        continue;
    }
    try {
        $wire = json_decode(
            file_get_contents($caseDirectory . '/' . $scenario['json']),
            true,
            512,
            JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING,
        );
        $payload = Protocol::expectPayload($wire, 'SearchResult');
        SearchResult::fromPayload($payload);
    } catch (Throwable) {
        $outcomes[$scenario['id']] = 'client';
        continue;
    }
    throw new RuntimeException('malformed result was accepted: ' . $scenario['id']);
}
$traces['malformed_outcomes'] = $outcomes;
file_put_contents($traceDirectory . '/php.json', json_encode($traces, JSON_THROW_ON_ERROR));
