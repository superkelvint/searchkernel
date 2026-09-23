<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

final class SearchResult
{
    /**
     * @param list<Hit> $hits
     * @param list<ResultGroup> $groups
     * @param list<ResultIssue> $errors
     * @param array<string,mixed> $metadata
     * @param array<string,mixed> $aggregations
     * @param array<string,string> $continuations
     */
    public function __construct(
        public readonly Hit|ResultGroup|null $root,
        public readonly array $hits,
        public readonly array $groups,
        public readonly int|string $totalHits,
        public readonly string $totalHitsRelation,
        public readonly ?Coverage $coverage,
        public readonly ?SearchTiming $timing,
        public readonly array $errors,
        public readonly mixed $trace,
        public readonly array $metadata,
        public readonly array $aggregations,
        public readonly array $continuations,
    ) {}

    /** @param array<string, mixed> $payload */
    public static function fromPayload(array $payload): self
    {
        $root = null;
        if (array_key_exists('root', $payload) && $payload['root'] !== null) {
            if (!is_array($payload['root'])) {
                throw new UnexpectedValueException('SearchResult.root must be a ResultNode object');
            }
            $root = ResultGroup::decodeNode($payload['root']);
        }

        if ($root instanceof ResultGroup) {
            $hits = $root->hits;
            $groups = $root->groups;
            $aggregations = $root->fields;
            $continuations = $root->continuations;
        } elseif ($root instanceof Hit) {
            $hits = [$root];
            $groups = [];
            $aggregations = [];
            $continuations = [];
        } else {
            $hits = [];
            $groups = [];
            $aggregations = [];
            $continuations = [];
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

        $coverage = is_array($payload['coverage'] ?? null) ? new Coverage($payload['coverage']) : null;
        $timing = is_array($payload['timing'] ?? null) ? new SearchTiming($payload['timing']) : null;

        $errors = [];
        foreach ($payload['errors'] ?? [] as $error) {
            if (!is_array($error)) {
                throw new UnexpectedValueException('SearchResult.errors must contain objects');
            }
            $errors[] = new ResultIssue($error);
        }

        $trace = null;
        if (array_key_exists('trace', $payload) && $payload['trace'] !== null) {
            if (!is_array($payload['trace'])) {
                throw new UnexpectedValueException('SearchResult.trace must be a portable Value');
            }
            $trace = Protocol::decodeValue($payload['trace']);
        }

        return new self(
            $root,
            $hits,
            $groups,
            $totalHits,
            $relation,
            $coverage,
            $timing,
            $errors,
            $trace,
            Protocol::decodeValueEntries($payload['metadata'] ?? []),
            $aggregations,
            $continuations,
        );
    }
}
