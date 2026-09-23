<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

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
                throw new UnexpectedValueException('hit relevance must be an object');
            }
            $relevance = Protocol::decodeRelevance($node['relevance']);
        }

        $sortValues = [];
        foreach ($node['sort_values'] ?? [] as $value) {
            if (!is_array($value)) {
                throw new UnexpectedValueException('sort values must contain portable Value objects');
            }
            $sortValues[] = Protocol::decodeValue($value);
        }

        $highlights = [];
        foreach ($node['highlights'] ?? [] as $highlight) {
            if (!is_array($highlight)) {
                throw new UnexpectedValueException('highlight must be an object');
            }
            $fragments = [];
            foreach ($highlight['fragments'] ?? [] as $fragment) {
                if (!is_array($fragment)) {
                    throw new UnexpectedValueException('highlight fragment must be an object');
                }
                $matches = [];
                foreach ($fragment['matches'] ?? [] as $match) {
                    if (!is_array($match)) {
                        throw new UnexpectedValueException('highlight match must be an object');
                    }
                    $matches[] = [
                        (int) ($match['start_byte'] ?? 0),
                        (int) ($match['end_byte'] ?? 0),
                    ];
                }
                $fragments[] = [
                    'text' => (string) ($fragment['text'] ?? ''),
                    'matches' => $matches,
                ];
            }
            $highlights[] = [
                'field' => (string) ($highlight['field'] ?? ''),
                'fragments' => $fragments,
            ];
        }

        return new self(
            (string) ($node['id'] ?? ''),
            Protocol::decodeValueEntries($node['fields'] ?? []),
            $relevance,
            isset($node['source']) ? (string) $node['source'] : null,
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
        if (!is_array($values)) {
            throw new UnexpectedValueException('expected a string array');
        }
        foreach ($values as $value) {
            if (!is_string($value)) {
                throw new UnexpectedValueException('expected a string array');
            }
        }
        return array_values($values);
    }
}
