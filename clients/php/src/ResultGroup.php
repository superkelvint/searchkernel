<?php

declare(strict_types=1);

namespace SearchKernel;


final class ResultGroup
{
    /** @param array<string,mixed> $node */
    private function __construct(array $node)
    {
        $this->id = self::optionalString($node, 'id');
        $this->label = self::optionalString($node, 'label');
        if (array_key_exists('value', $node) && $node['value'] !== null && !is_array($node['value'])) {
            throw new ProtocolException('ResultGroup.value must be a portable Value');
        }
        $this->value = is_array($node['value'] ?? null) ? Protocol::decodeValue($node['value']) : null;
        $fieldEntries = Protocol::listField($node, 'fields', 'ResultGroup.fields');
        $this->fields = Protocol::decodeValueEntries($fieldEntries);
        $fieldKinds = [];
        foreach ($fieldEntries as $entry) {
            if (!is_array($entry)
                || !is_string($entry['key'] ?? null)
                || !is_array($entry['value'] ?? null)
                || !is_string($entry['value']['kind_type'] ?? null)
            ) {
                throw new ProtocolException('malformed ResultGroup field entry');
            }
            $fieldKinds[$entry['key']] = $entry['value']['kind_type'];
        }
        $this->fieldKinds = $fieldKinds;
        $this->source = self::optionalString($node, 'source');
        $this->types = self::strings(
            Protocol::listField($node, 'types', 'ResultGroup.types'),
        );
        if (array_key_exists('relevance', $node) && $node['relevance'] !== null && !is_array($node['relevance'])) {
            throw new ProtocolException('ResultGroup.relevance must be an object');
        }
        $this->relevance = is_array($node['relevance'] ?? null)
            ? Protocol::decodeRelevance($node['relevance'])
            : null;
        $this->metadata = Protocol::decodeValueEntries(
            Protocol::listField($node, 'metadata', 'ResultGroup.metadata'),
        );

        $limits = $node['limits'] ?? null;
        if ($limits !== null && !is_array($limits)) {
            throw new ProtocolException('ResultGroup.limits must be an object');
        }
        if (is_array($limits)) {
            foreach (['from', 'to'] as $key) {
                if (array_key_exists($key, $limits)
                    && $limits[$key] !== null
                    && !is_array($limits[$key])
                ) {
                    throw new ProtocolException('ResultGroup.limits.' . $key . ' must be a portable Value');
                }
            }
        }
        $this->limits = is_array($limits) ? [
            'from' => is_array($limits['from'] ?? null) ? Protocol::decodeValue($limits['from']) : null,
            'to' => is_array($limits['to'] ?? null) ? Protocol::decodeValue($limits['to']) : null,
        ] : null;

        $continuationEntries = Protocol::listField(
            $node,
            'continuations',
            'ResultGroup.continuations',
        );
        $continuations = [];
        foreach ($continuationEntries as $entry) {
            if (!is_array($entry) || !is_string($entry['key'] ?? null) || !is_string($entry['value'] ?? null)) {
                throw new ProtocolException('malformed continuation entry');
            }
            if (array_key_exists($entry['key'], $continuations)) {
                throw new ProtocolException('duplicate continuation key: ' . $entry['key']);
            }
            $continuations[$entry['key']] = $entry['value'];
        }
        $this->continuations = $continuations;

        $childEntries = Protocol::listField($node, 'children', 'ResultGroup.children');
        $children = [];
        foreach ($childEntries as $child) {
            if (!is_array($child)) {
                throw new ProtocolException('malformed ResultGroup child');
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
    /** @var array<string,string> */
    private readonly array $fieldKinds;
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

    /** @internal */
    public function fieldKind(string $key): ?string
    {
        return $this->fieldKinds[$key] ?? null;
    }

    /** @param array<string,mixed> $node */
    public static function decodeNode(array $node): Hit|self
    {
        $type = $node['node_type'] ?? null;
        $payload = $node['node'] ?? null;
        if (!is_string($type) || !is_array($payload)) {
            throw new ProtocolException('malformed ResultNode union');
        }
        return match ($type) {
            'Hit' => Hit::fromJson($payload),
            'ResultGroup' => new self($payload),
            default => throw new ProtocolException('unsupported ResultNode kind: ' . $type),
        };
    }

    /** @param array<string,mixed> $node */
    private static function optionalString(array $node, string $key): ?string
    {
        if (!array_key_exists($key, $node) || $node[$key] === null) {
            return null;
        }
        if (!is_string($node[$key])) {
            throw new ProtocolException('ResultGroup.' . $key . ' must be a string');
        }
        return $node[$key];
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
