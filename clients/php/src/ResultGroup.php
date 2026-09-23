<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

final class ResultGroup
{
    /** @param array<string,mixed> $node */
    private function __construct(array $node)
    {
        $this->id = isset($node['id']) ? (string) $node['id'] : null;
        $this->label = isset($node['label']) ? (string) $node['label'] : null;
        $this->value = is_array($node['value'] ?? null) ? Protocol::decodeValue($node['value']) : null;
        $this->fields = Protocol::decodeValueEntries($node['fields'] ?? []);
        $this->source = isset($node['source']) ? (string) $node['source'] : null;
        $this->types = self::strings($node['types'] ?? []);
        $this->relevance = is_array($node['relevance'] ?? null)
            ? Protocol::decodeRelevance($node['relevance'])
            : null;
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);

        $limits = $node['limits'] ?? null;
        if ($limits !== null && !is_array($limits)) {
            throw new UnexpectedValueException('ResultGroup.limits must be an object');
        }
        $this->limits = is_array($limits) ? [
            'from' => is_array($limits['from'] ?? null) ? Protocol::decodeValue($limits['from']) : null,
            'to' => is_array($limits['to'] ?? null) ? Protocol::decodeValue($limits['to']) : null,
        ] : null;

        $continuations = [];
        foreach ($node['continuations'] ?? [] as $entry) {
            if (!is_array($entry) || !is_string($entry['key'] ?? null) || !is_string($entry['value'] ?? null)) {
                throw new UnexpectedValueException('malformed continuation entry');
            }
            if (array_key_exists($entry['key'], $continuations)) {
                throw new UnexpectedValueException('duplicate continuation key: ' . $entry['key']);
            }
            $continuations[$entry['key']] = $entry['value'];
        }
        $this->continuations = $continuations;

        $children = [];
        foreach ($node['children'] ?? [] as $child) {
            if (!is_array($child)) {
                throw new UnexpectedValueException('malformed ResultGroup child');
            }
            $children[] = self::decodeNode($child);
        }
        $this->children = $children;
        $this->hits = array_values(array_filter($children, static fn ($child): bool => $child instanceof Hit));
        $this->groups = array_values(array_filter($children, static fn ($child): bool => $child instanceof self));
    }

    public readonly ?string $id;
    public readonly ?string $label;
    public readonly mixed $value;
    /** @var array<string,mixed> */
    public readonly array $fields;
    /** @var list<Hit|ResultGroup> */
    public readonly array $children;
    /** @var list<ResultGroup> */
    public readonly array $groups;
    /** @var list<Hit> */
    public readonly array $hits;
    /** @var array<string,string> */
    public readonly array $continuations;
    public readonly ?string $source;
    /** @var list<string> */
    public readonly array $types;
    public readonly ?float $relevance;
    /** @var array{from:mixed,to:mixed}|null */
    public readonly ?array $limits;
    /** @var array<string,mixed> */
    public readonly array $metadata;

    /** @param array<string,mixed> $node */
    public static function decodeNode(array $node): Hit|self
    {
        $type = $node['node_type'] ?? null;
        $payload = $node['node'] ?? null;
        if (!is_string($type) || !is_array($payload)) {
            throw new UnexpectedValueException('malformed ResultNode union');
        }
        return match ($type) {
            'Hit' => Hit::fromJson($payload),
            'ResultGroup' => new self($payload),
            default => throw new UnexpectedValueException('unsupported ResultNode kind: ' . $type),
        };
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
