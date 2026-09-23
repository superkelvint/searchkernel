<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class ValueObject
{
    /** @var list<array{key:string,value:mixed}> */
    public readonly array $entries;

    /** @param list<array{key:string,value:mixed}> $entries */
    public function __construct(array $entries = [])
    {
        $seen = [];
        foreach ($entries as $entry) {
            if (!is_array($entry) || !is_string($entry['key'] ?? null) || !array_key_exists('value', $entry)) {
                throw new InvalidArgumentException('ValueObject entries require string key and value');
            }
            if (array_key_exists($entry['key'], $seen)) {
                throw new InvalidArgumentException('duplicate ValueObject key: ' . $entry['key']);
            }
            $seen[$entry['key']] = true;
        }
        usort($entries, static fn (array $a, array $b): int => strcmp($a['key'], $b['key']));
        $this->entries = array_values($entries);
    }

    /** @param list<array{key:string,value:mixed}> $pairs */
    public static function fromPairs(array $pairs): self
    {
        return new self($pairs);
    }

    /** @param array<string,mixed> $values */
    public static function fromArray(array $values): self
    {
        $entries = [];
        foreach ($values as $key => $value) {
            if (!is_string($key)) {
                throw new InvalidArgumentException('ValueObject::fromArray requires string keys');
            }
            $entries[] = ['key' => $key, 'value' => $value];
        }
        return new self($entries);
    }
}
