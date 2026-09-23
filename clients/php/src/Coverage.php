<?php

declare(strict_types=1);

namespace SearchKernel;

final class Coverage
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->coveragePercent = self::optionalFloat($node, 'coverage_percent');
        $this->documents = self::optionalUInt64($node, 'documents');
        $this->full = self::optionalBool($node, 'full');
        $this->nodes = self::optionalUInt32($node, 'nodes');
        $this->results = self::optionalUInt64($node, 'results');
        $this->resultsFull = self::optionalUInt64($node, 'results_full');
        if (array_key_exists('degraded', $node) && $node['degraded'] !== null && !is_array($node['degraded'])) {
            throw new ProtocolException('coverage degraded must be an object');
        }
        $this->degraded = is_array($node['degraded'] ?? null)
            ? new CoverageDegraded($node['degraded'])
            : null;
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);
    }

    public readonly ?float $coveragePercent;
    public readonly int|string|null $documents;
    public readonly ?bool $full;
    public readonly ?int $nodes;
    public readonly int|string|null $results;
    public readonly int|string|null $resultsFull;
    public readonly ?CoverageDegraded $degraded;
    /** @var array<string,mixed> */
    public readonly array $metadata;

    /** @param array<string,mixed> $node */
    private static function wrapper(array $node, string $key): mixed
    {
        if (!array_key_exists($key, $node) || $node[$key] === null) {
            return null;
        }
        if (!is_array($node[$key]) || !array_key_exists('value', $node[$key])) {
            throw new ProtocolException($key . ' must be an optional-value object');
        }
        return $node[$key]['value'];
    }

    /** @param array<string,mixed> $node */
    private static function optionalFloat(array $node, string $key): ?float
    {
        $value = self::wrapper($node, $key);
        if ($value === null) return null;
        if ((!is_int($value) && !is_float($value)) || !is_finite((float) $value)) {
            throw new ProtocolException($key . ' must contain a finite number');
        }
        return (float) $value;
    }

    /** @param array<string,mixed> $node */
    private static function optionalBool(array $node, string $key): ?bool
    {
        $value = self::wrapper($node, $key);
        if ($value === null) return null;
        if (!is_bool($value)) {
            throw new ProtocolException($key . ' must contain a boolean');
        }
        return $value;
    }

    /** @param array<string,mixed> $node */
    private static function optionalUInt32(array $node, string $key): ?int
    {
        $value = self::wrapper($node, $key);
        if ($value === null) return null;
        if (!is_int($value) || $value < 0 || $value > 0xFFFF_FFFF) {
            throw new ProtocolException($key . ' must contain a uint32');
        }
        return $value;
    }

    /** @param array<string,mixed> $node */
    private static function optionalUInt64(array $node, string $key): int|string|null
    {
        $value = self::wrapper($node, $key);
        if ($value === null) return null;
        return Protocol::decodeValue([
            'kind_type' => 'UInt64Value',
            'kind' => ['value' => $value],
        ]);
    }
}
