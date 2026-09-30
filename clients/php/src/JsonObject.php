<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

/**
 * Explicit standard-JSON object marker for FlatBuffers tables.
 *
 * PHP arrays cannot distinguish an empty JSON object from an empty JSON list.
 * This marker is wire representation only; it does not carry Search Kernel
 * domain semantics.
 *
 * @internal
 */
final class JsonObject
{
    /** @var array<string,mixed> */
    public readonly array $fields;

    /** @param array<string,mixed> $fields */
    public function __construct(array $fields = [])
    {
        foreach ($fields as $key => $_value) {
            if (!is_string($key)) {
                throw new InvalidArgumentException('JSON object keys must be strings');
            }
        }
        ksort($fields, SORT_STRING);
        $this->fields = $fields;
    }

    public static function empty(): self
    {
        return new self();
    }
}
