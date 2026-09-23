<?php

declare(strict_types=1);

namespace SearchKernel;


final class Hit
{
    /** @param array<string, mixed> $fields
     *  @param array<string,mixed> $matchFeatures
     *  @param array<string,mixed> $summaryFeatures
     *  @param array<string,mixed> $rankFeatures
     *  @param list<mixed> $sortValues
     *  @param list<array<string,mixed>> $highlights
     *  @param list<string> $types
     */
    public function __construct(
        public readonly string $id,
        public readonly array $fields,
        public readonly ?float $relevance,
        public readonly ?string $source = null,
        public readonly array $types = [],
        public readonly array $matchFeatures = [],
        public readonly array $summaryFeatures = [],
        public readonly array $rankFeatures = [],
        public readonly array $sortValues = [],
        public readonly array $highlights = [],
    ) {}

    public function score(): ?float
    {
        return $this->relevance;
    }

    /** @param array<string, mixed> $node */
    public static function fromJson(array $node): self
    {
        $relevance = null;
        if (array_key_exists('relevance', $node) && $node['relevance'] !== null) {
            if (!is_array($node['relevance'])) {
                throw new ProtocolException('hit relevance must be an object');
            }
            $relevance = Protocol::decodeRelevance($node['relevance']);
        }

        $id = $node['id'] ?? null;
        if (!is_string($id)) {
            throw new ProtocolException('Hit.id must be a string');
        }
        if (array_key_exists('source', $node) && $node['source'] !== null && !is_string($node['source'])) {
            throw new ProtocolException('Hit.source must be a string when present');
        }

        $sortValuesNode = $node['sort_values'] ?? [];
        if (!is_array($sortValuesNode) || !array_is_list($sortValuesNode)) {
            throw new ProtocolException('Hit.sort_values must be a list');
        }
        $sortValues = [];
        foreach ($sortValuesNode as $value) {
            if (!is_array($value)) {
                throw new ProtocolException('sort values must contain portable Value objects');
            }
            $sortValues[] = Protocol::decodeValue($value);
        }

        $highlightNodes = $node['highlights'] ?? [];
        if (!is_array($highlightNodes) || !array_is_list($highlightNodes)) {
            throw new ProtocolException('Hit.highlights must be a list');
        }
        $highlights = [];
        foreach ($highlightNodes as $highlight) {
            if (!is_array($highlight)) {
                throw new ProtocolException('highlight must be an object');
            }
            if (!is_string($highlight['field'] ?? null)
                || !is_array($highlight['fragments'] ?? [])
                || !array_is_list($highlight['fragments'] ?? [])
            ) {
                throw new ProtocolException('malformed highlight result');
            }
            $fragments = [];
            foreach ($highlight['fragments'] ?? [] as $fragment) {
                if (!is_array($fragment)) {
                    throw new ProtocolException('highlight fragment must be an object');
                }
                if (!is_string($fragment['text'] ?? null)
                    || !is_array($fragment['matches'] ?? [])
                    || !array_is_list($fragment['matches'] ?? [])
                ) {
                    throw new ProtocolException('malformed highlight fragment');
                }
                $matches = [];
                foreach ($fragment['matches'] ?? [] as $match) {
                    if (!is_array($match)) {
                        throw new ProtocolException('highlight match must be an object');
                    }
                    $start = $match['start_byte'] ?? null;
                    $end = $match['end_byte'] ?? null;
                    if (!is_int($start) || !is_int($end)
                        || $start < 0 || $end < 0
                        || $start > 0xFFFF_FFFF || $end > 0xFFFF_FFFF
                    ) {
                        throw new ProtocolException('highlight spans must contain uint32 offsets');
                    }
                    $matches[] = [$start, $end];
                }
                $fragments[] = [
                    'text' => $fragment['text'],
                    'matches' => $matches,
                ];
            }
            $highlights[] = [
                'field' => $highlight['field'],
                'fragments' => $fragments,
            ];
        }

        return new self(
            $id,
            Protocol::decodeValueEntries($node['fields'] ?? []),
            $relevance,
            $node['source'] ?? null,
            self::strings($node['types'] ?? []),
            Protocol::decodeValueEntries($node['match_features'] ?? []),
            Protocol::decodeValueEntries($node['summary_features'] ?? []),
            Protocol::decodeValueEntries($node['rank_features'] ?? []),
            $sortValues,
            $highlights,
        );
    }

    public function get(string $field, mixed $default = null): mixed
    {
        return $this->fields[$field] ?? $default;
    }

    /** @return list<string> */
    private static function strings(mixed $values): array
    {
        if (!is_array($values) || !array_is_list($values)) {
            throw new ProtocolException('expected a string list');
        }
        foreach ($values as $value) {
            if (!is_string($value)) {
                throw new ProtocolException('expected a string array');
            }
        }
        return array_values($values);
    }
}
