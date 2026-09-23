<?php

declare(strict_types=1);

namespace SearchKernel;


final class SearchResult
{
    /** @var array<string,list<array{value:mixed,count:int|string}>> */
    public array $facets = [];
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

    public function projectFacet(string $resultName, string $expectedField): void
    {
        $candidates = array_values(array_filter(
            $this->groups,
            static fn (ResultGroup $group): bool =>
                ($group->id === $expectedField || $group->id === null)
                && $group->label === $resultName
                && $group->value === null
                && $group->fields === []
                && $group->continuations === []
                && $group->source === null
                && $group->types === []
                && $group->relevance === null
                && $group->limits === null
                && $group->metadata === []
                && count($group->groups) === count($group->children),
        ));
        if (count($candidates) !== 1) {
            return;
        }

        $buckets = [];
        foreach ($candidates[0]->groups as $child) {
            $optimized = $child->id === null && $child->label === null;
            $generic = $child->id === 'grouping-0' && $child->label === $resultName;
            if ((!$optimized && !$generic)
                || $child->children !== []
                || $child->continuations !== []
                || $child->source !== null
                || $child->types !== []
                || $child->relevance !== null
                || $child->limits !== null
                || $child->metadata !== []
                || count($child->fields) !== 1
                || !array_key_exists('count', $child->fields)
            ) {
                return;
            }
            $count = $child->fields['count'];
            if (!is_int($count) && !is_string($count)) {
                return;
            }
            if ($generic && is_int($count) && $count < 0) {
                return;
            }
            $buckets[] = ['value' => $child->value, 'count' => $count];
        }

        $this->facets[$resultName] = $buckets;
    }

    /** @param array<string, mixed> $payload */
    public static function fromPayload(array $payload): self
    {
        $root = null;
        if (array_key_exists('root', $payload) && $payload['root'] !== null) {
            if (!is_array($payload['root'])) {
                throw new ProtocolException('SearchResult.root must be a ResultNode object');
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
        if (array_key_exists('total_hits', $payload) && $payload['total_hits'] !== null) {
            if (!is_array($payload['total_hits'])) {
                throw new ProtocolException('SearchResult.total_hits must be an object');
            }
            $totalHits = array_key_exists('value', $payload['total_hits'])
                ? Protocol::decodeUInt64Scalar($payload['total_hits']['value'], 'TotalHits.value')
                : 0;
            $relation = array_key_exists('relation', $payload['total_hits'])
                ? Protocol::decodeStringScalar($payload['total_hits']['relation'], 'TotalHits.relation')
                : 'Unknown';
        }

        if (array_key_exists('coverage', $payload) && $payload['coverage'] !== null && !is_array($payload['coverage'])) {
            throw new ProtocolException('SearchResult.coverage must be an object');
        }
        if (array_key_exists('timing', $payload) && $payload['timing'] !== null && !is_array($payload['timing'])) {
            throw new ProtocolException('SearchResult.timing must be an object');
        }
        $coverage = is_array($payload['coverage'] ?? null) ? new Coverage($payload['coverage']) : null;
        $timing = is_array($payload['timing'] ?? null) ? new SearchTiming($payload['timing']) : null;

        $errorEntries = $payload['errors'] ?? [];
        if (!is_array($errorEntries)) {
            throw new ProtocolException('SearchResult.errors must be an array');
        }
        $errors = [];
        foreach ($errorEntries as $error) {
            if (!is_array($error)) {
                throw new ProtocolException('SearchResult.errors must contain objects');
            }
            $errors[] = new ResultIssue($error);
        }

        $trace = null;
        if (array_key_exists('trace', $payload) && $payload['trace'] !== null) {
            if (!is_array($payload['trace'])) {
                throw new ProtocolException('SearchResult.trace must be a portable Value');
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
