<?php

declare(strict_types=1);

namespace SearchKernel;

final class PatchBuilder
{
    /** @var list<array<string,mixed>> */
    private array $operations = [];

    public function __construct(
        private readonly Index $index,
        private readonly string $id,
    ) {}

    public function set(string $field, mixed $value): self
    {
        $copy = clone $this;
        $copy->operations[] = [
            'operation_type' => 'SetField',
            'operation' => ['field' => $field, 'value' => Protocol::value($value)],
        ];
        return $copy;
    }

    public function unset(string $field): self
    {
        $copy = clone $this;
        $copy->operations[] = [
            'operation_type' => 'UnsetField',
            'operation' => ['field' => $field],
        ];
        return $copy;
    }

    public function execute(): string
    {
        return $this->index->executePatch($this->id, $this->operations);
    }
}
