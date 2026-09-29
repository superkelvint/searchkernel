<?php
declare(strict_types=1);

spl_autoload_register(static function (string $class): void {
    $prefix = 'SearchKernel\\';
    if (str_starts_with($class, $prefix)) {
        $path = dirname(__DIR__) . '/src/' . substr($class, strlen($prefix)) . '.php';
        if (is_file($path)) require $path;
    }
});

use SearchKernel\ProtocolException;
use SearchKernel\SearchResult;

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
    if (!is_array($wire['payload'] ?? null)) {
        throw new RuntimeException('SearchResult payload missing: ' . $scenario['id']);
    }
    $result = SearchResult::fromPayload($wire['payload']);
    if ($result->root === null) throw new RuntimeException('SearchResult root missing: ' . $scenario['id']);
    $traces[$scenario['id']] = file_get_contents($caseDirectory . '/' . $scenario['json']);
}

$rejected = [];
foreach ($cases['malformed'] as $scenario) {
    if ($scenario['bridge_rejected']) {
        $rejected[] = $scenario['id'];
        continue;
    }
    try {
        $wire = json_decode(
            file_get_contents($caseDirectory . '/' . $scenario['json']),
            true,
            512,
            JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING,
        );
        SearchResult::fromPayload($wire['payload']);
    } catch (Throwable) {
        $rejected[] = $scenario['id'];
        continue;
    }
    throw new RuntimeException('malformed result was accepted: ' . $scenario['id']);
}
$traces['malformed_rejected'] = $rejected;
file_put_contents($traceDirectory . '/php.json', json_encode($traces, JSON_THROW_ON_ERROR));
