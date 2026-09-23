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

        $sortValuesNode = Protocol::listField($node, 'sort_values', 'Hit.sort_values');
        $sortValues = [];
        foreach ($sortValuesNode as $value) {
            if (!is_array($value)) {
                throw new ProtocolException('sort values must contain portable Value objects');
            }
            $sortValues[] = Protocol::decodeValue($value);
        }

        $highlightNodes = Protocol::listField($node, 'highlights', 'Hit.highlights');
        $highlights = [];
        foreach ($highlightNodes as $highlight) {
            if (!is_array($highlight)) {
                throw new ProtocolException('highlight must be an object');
            }
            if (!is_string($highlight['field'] ?? null)) {
                throw new ProtocolException('malformed highlight result');
            }
            $fragments = [];
            foreach (Protocol::listField(
                $highlight,
                'fragments',
                'HighlightResult.fragments',
            ) as $fragment) {
                if (!is_array($fragment)) {
                    throw new ProtocolException('highlight fragment must be an object');
                }
                if (!is_string($fragment['text'] ?? null)) {
                    throw new ProtocolException('malformed highlight fragment');
                }
                $matches = [];
                foreach (Protocol::listField(
                    $fragment,
                    'matches',
                    'HighlightFragment.matches',
                ) as $match) {
                    if (!is_array($match)) {
                        throw new ProtocolException('highlight match must be an object');
                    }
                    $matches[] = [
                        Protocol::uint32ScalarField(
                            $match,
                            'start_byte',
                            'HighlightSpan.start_byte',
                        ),
                        Protocol::uint32ScalarField(
                            $match,
                            'end_byte',
                            'HighlightSpan.end_byte',
                        ),
                    ];
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
            Protocol::decodeValueEntries(
                Protocol::listField($node, 'fields', 'Hit.fields'),
            ),
            $relevance,
            $node['source'] ?? null,
            self::strings(Protocol::listField($node, 'types', 'Hit.types')),
            Protocol::decodeValueEntries(
                Protocol::listField($node, 'match_features', 'Hit.match_features'),
            ),
            Protocol::decodeValueEntries(
                Protocol::listField($node, 'summary_features', 'Hit.summary_features'),
            ),
            Protocol::decodeValueEntries(
                Protocol::listField($node, 'rank_features', 'Hit.rank_features'),
            ),
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
