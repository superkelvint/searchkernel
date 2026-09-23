<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

final class CoverageDegraded
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->matchPhase = self::optional($node, 'match_phase');
        $this->timeout = self::optional($node, 'timeout');
        $this->adaptiveTimeout = self::optional($node, 'adaptive_timeout');
        $this->annTimeout = self::optional($node, 'ann_timeout');
        $this->nonIdealState = self::optional($node, 'non_ideal_state');
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);
    }

    public readonly mixed $matchPhase;
    public readonly mixed $timeout;
    public readonly mixed $adaptiveTimeout;
    public readonly mixed $annTimeout;
    public readonly mixed $nonIdealState;
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

final class Coverage
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->coveragePercent = self::optional($node, 'coverage_percent');
        $this->documents = self::optional($node, 'documents');
        $this->full = self::optional($node, 'full');
        $this->nodes = self::optional($node, 'nodes');
        $this->results = self::optional($node, 'results');
        $this->resultsFull = self::optional($node, 'results_full');
        $this->degraded = is_array($node['degraded'] ?? null)
            ? new CoverageDegraded($node['degraded'])
            : null;
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);
    }

    public readonly mixed $coveragePercent;
    public readonly mixed $documents;
    public readonly mixed $full;
    public readonly mixed $nodes;
    public readonly mixed $results;
    public readonly mixed $resultsFull;
    public readonly ?CoverageDegraded $degraded;
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
