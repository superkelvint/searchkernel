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

use SearchKernel\Index;
use SearchKernel\Protocol;
use SearchKernel\SearchKernelException;
use SearchKernel\SearchResult;
use SearchKernel\Tensor;
use SearchKernel\Transport;

function check(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

final class PrintedResponseTransport implements Transport
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

$name = $argv[1] ?? '';
$json = stream_get_contents(STDIN);
if (!is_string($json) || $json === '') {
    throw new RuntimeException('missing BFBS-printed response JSON');
}

$envelope = json_decode($json, true, 512, JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING);
if (!is_array($envelope) || array_is_list($envelope)) {
    throw new RuntimeException('BFBS-printed response must be an object');
}

$payload = $envelope['payload'] ?? null;
if (!is_array($payload)) {
    throw new RuntimeException('BFBS-printed response is missing payload');
}

switch ($name) {
    case 'portable-error':
    case 'default-error-retryable-false':
        try {
            Protocol::expectPayload($envelope, 'SearchResult');
            throw new RuntimeException('canonical Error did not throw');
        } catch (SearchKernelException $error) {
            check($error->errorCode === 'InvalidArgument', 'canonical Error code changed');
            check($error->retryable === false, 'omitted Error.retryable did not decode as false');
        }
        break;

    case 'recursive-search-result':
        $result = SearchResult::fromPayload($payload);
        check($result->totalHits === 1, 'recursive result total hits changed');
        check(count($result->hits) === 1, 'recursive result hit disappeared');
        break;

    case 'default-delete-false':
        $index = new Index(new PrintedResponseTransport($envelope));
        check($index->delete('bfbs-doc') === false, 'omitted DeleteResult.deleted did not decode as false');
        break;

    case 'default-analyze-zero':
        $index = new Index(new PrintedResponseTransport($envelope));
        $tokens = $index->analyze('fox')->execute();
        check(count($tokens) === 1, 'analysis token missing');
        check($tokens[0]['position'] === 0, 'omitted AnalyzedToken.position did not decode as zero');
        check($tokens[0]['start_byte'] === 0, 'omitted AnalyzedToken.start_byte did not decode as zero');
        check($tokens[0]['end_byte'] === 3, 'analysis end byte changed');
        break;

    case 'default-highlight-zero':
        $result = SearchResult::fromPayload($payload);
        check(count($result->hits) === 1, 'highlight result hit missing');
        check(
            $result->hits[0]->highlights[0]['fragments'][0]['matches'][0] === [0, 3],
            'omitted HighlightSpan.start_byte did not decode as zero',
        );
        break;

    case 'default-relevance-zero':
        $result = SearchResult::fromPayload($payload);
        check(count($result->hits) === 1, 'relevance result hit missing');
        check($result->hits[0]->score() === 0.0, 'omitted FiniteRelevance.value did not decode as zero');
        break;

    case 'portable-zero-values':
        $result = SearchResult::fromPayload($payload);
        check(count($result->hits) === 1, 'zero-value result hit missing');
        $fields = $result->hits[0]->fields;
        check(($fields['bool'] ?? null) === false, 'omitted BoolValue.value did not decode as false');
        check(($fields['int'] ?? null) === 0, 'omitted Int64Value.value did not decode as zero');
        check(($fields['uint'] ?? null) === 0, 'omitted UInt64Value.value did not decode as zero');
        check(($fields['float'] ?? null) === 0.0, 'omitted Float64Value.value did not decode as zero');
        break;

    case 'default-sparse-tensor-zero':
        $document = $payload['document'] ?? null;
        check(is_array($document), 'tensor GetResult document missing');
        $decoded = Protocol::decodeDocument($document);
        $tensor = $decoded['embedding'] ?? null;
        check($tensor instanceof Tensor, 'sparse tensor did not decode as Tensor');
        check(
            ($tensor->payload['cells'][0]['value'] ?? null) === 0.0,
            'omitted TensorCell.value did not decode as zero',
        );
        break;

    default:
        throw new RuntimeException('unknown BFBS response fixture: ' . $name);
}

echo "PASS ", $name, PHP_EOL;
