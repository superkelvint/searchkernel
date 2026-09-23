<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

final class SearchResult
{
    /** @param list<Hit> $hits */
    public function __construct(
        public readonly array $hits,
        public readonly int|string $totalHits,
        public readonly string $totalHitsRelation,
    ) {}

    /** @param array<string, mixed> $payload */
    public static function fromPayload(array $payload): self
    {
        $hits = [];
        if (is_array($payload['root'] ?? null)) {
            self::collectTopLevelHits($payload['root'], $hits);
        }

        $totalHits = 0;
        $relation = 'Unknown';
        if (is_array($payload['total_hits'] ?? null)) {
            $totalHits = $payload['total_hits']['value'] ?? 0;
            if (!is_int($totalHits) && !is_string($totalHits)) {
                throw new UnexpectedValueException('total_hits.value must be an integer or lossless decimal string');
            }
            $relation = (string) ($payload['total_hits']['relation'] ?? 'Unknown');
        }

        return new self($hits, $totalHits, $relation);
    }

    /** @param array<string, mixed> $node
     *  @param list<Hit> $hits
     */
    private static function collectTopLevelHits(array $node, array &$hits): void
    {
        $type = $node['node_type'] ?? null;
        $payload = $node['node'] ?? null;
        if (!is_string($type) || !is_array($payload)) {
            throw new UnexpectedValueException('malformed ResultNode union');
        }

        if ($type === 'Hit') {
            $hits[] = Hit::fromJson($payload);
            return;
        }
        if ($type !== 'ResultGroup') {
            throw new UnexpectedValueException('unsupported ResultNode kind: ' . $type);
        }

        $children = $payload['children'] ?? [];
        if (!is_array($children)) {
            throw new UnexpectedValueException('ResultGroup children must be an array');
        }
        foreach ($children as $child) {
            if (!is_array($child)) {
                throw new UnexpectedValueException('malformed ResultGroup child');
            }
            if (($child['node_type'] ?? null) === 'Hit') {
                $childPayload = $child['node'] ?? null;
                if (!is_array($childPayload)) {
                    throw new UnexpectedValueException('missing Hit union payload');
                }
                $hits[] = Hit::fromJson($childPayload);
            }
        }
    }
}
