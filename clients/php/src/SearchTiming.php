<?php

declare(strict_types=1);

namespace SearchKernel;

final class SearchTiming
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->queryTimeSeconds = self::optionalFloat($node, 'query_time_seconds');
        $this->searchTimeSeconds = self::optionalFloat($node, 'search_time_seconds');
        $this->summaryFetchTimeSeconds = self::optionalFloat($node, 'summary_fetch_time_seconds');
        $this->metadata = Protocol::decodeValueEntries(
            Protocol::listField($node, 'metadata', 'SearchTiming.metadata'),
        );
    }

    public readonly ?float $queryTimeSeconds;
    public readonly ?float $searchTimeSeconds;
    public readonly ?float $summaryFetchTimeSeconds;
    /** @var array<string,mixed> */
    public readonly array $metadata;

    /** @param array<string,mixed> $node */
    private static function optionalFloat(array $node, string $key): ?float
    {
        if (!array_key_exists($key, $node) || $node[$key] === null) {
            return null;
        }
        if (!is_array($node[$key]) || !array_key_exists('value', $node[$key])) {
            throw new ProtocolException($key . ' must be an optional-value object');
        }
        $value = $node[$key]['value'];
        if ((!is_int($value) && !is_float($value)) || !is_finite((float) $value)) {
            throw new ProtocolException($key . ' must contain a finite number');
        }
        return (float) $value;
    }
}
