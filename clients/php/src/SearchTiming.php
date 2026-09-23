<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

final class SearchTiming
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->queryTimeSeconds = self::optional($node, 'query_time_seconds');
        $this->searchTimeSeconds = self::optional($node, 'search_time_seconds');
        $this->summaryFetchTimeSeconds = self::optional($node, 'summary_fetch_time_seconds');
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);
    }

    public readonly mixed $queryTimeSeconds;
    public readonly mixed $searchTimeSeconds;
    public readonly mixed $summaryFetchTimeSeconds;
    /** @var array<string,mixed> */
    public readonly array $metadata;

    /** @param array<string,mixed> $node */
    private static function optional(array $node, string $key): mixed
    {
        if (!array_key_exists($key, $node) || $node[$key] === null) {
            return null;
        }
        if (!is_array($node[$key]) || !array_key_exists('value', $node[$key])) {
            throw new UnexpectedValueException($key . ' must be an optional-value object');
        }
        return $node[$key]['value'];
    }
}
