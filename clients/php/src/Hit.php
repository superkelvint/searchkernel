<?php

declare(strict_types=1);

namespace SearchKernel;


final class Hit
{
    /** @param array<string, mixed> $fields */
    public function __construct(
        public readonly string $id,
        public readonly array $fields,
        public readonly ?float $relevance,
    ) {}

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
        if (!is_string($id) || $id === '') {
            throw new ProtocolException('Hit.id must be a non-empty string');
        }

        return new self(
            $id,
            Protocol::decodeValueEntries($node['fields'] ?? []),
            $relevance,
        );
    }

    public function get(string $field, mixed $default = null): mixed
    {
        return $this->fields[$field] ?? $default;
    }
}
