<?php

declare(strict_types=1);

namespace SearchKernel;

final class CoverageDegraded
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->matchPhase = self::optionalBool($node, 'match_phase');
        $this->timeout = self::optionalBool($node, 'timeout');
        $this->adaptiveTimeout = self::optionalBool($node, 'adaptive_timeout');
        $this->annTimeout = self::optionalBool($node, 'ann_timeout');
        $this->nonIdealState = self::optionalBool($node, 'non_ideal_state');
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);
    }

    public readonly ?bool $matchPhase;
    public readonly ?bool $timeout;
    public readonly ?bool $adaptiveTimeout;
    public readonly ?bool $annTimeout;
    public readonly ?bool $nonIdealState;
    /** @var array<string,mixed> */
    public readonly array $metadata;

    /** @param array<string,mixed> $node */
    private static function optionalBool(array $node, string $key): ?bool
    {
        if (!array_key_exists($key, $node) || $node[$key] === null) {
            return null;
        }
        if (!is_array($node[$key])
            || !array_key_exists('value', $node[$key])
            || !is_bool($node[$key]['value'])
        ) {
            throw new ProtocolException($key . ' must be an optional boolean');
        }
        return $node[$key]['value'];
    }
}

