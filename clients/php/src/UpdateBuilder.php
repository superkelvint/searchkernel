<?php

declare(strict_types=1);

namespace SearchKernel;

final class UpdateBuilder
{
    /** @var list<array<string,mixed>> */
    private array $operations = [];
    private ?string $condition = null;
    private ?bool $create = null;

    public function __construct(
        private readonly Index $index,
        private readonly string $id,
    ) {}

    public function set(string $path, mixed $value): self
    {
        return $this->operation($path, 'AssignUpdate', ['value' => Protocol::value($value)]);
    }

    public function assign(string $path, mixed $value): self
    {
        return $this->set($path, $value);
    }

    public function unset(string $path): self
    {
        return $this->operation($path, 'ClearUpdate', []);
    }

    public function clear(string $path): self
    {
        return $this->unset($path);
    }

    public function add(string $path, mixed $value): self
    {
        return $this->operation($path, 'AddUpdate', ['value' => Protocol::value($value)]);
    }

    public function remove(string $path, mixed $value): self
    {
        return $this->operation($path, 'RemoveUpdate', ['value' => Protocol::value($value)]);
    }

    public function increment(string $path, mixed $value): self
    {
        return $this->arithmetic($path, 'Increment', $value);
    }

    public function decrement(string $path, mixed $value): self
    {
        return $this->arithmetic($path, 'Decrement', $value);
    }

    public function multiply(string $path, mixed $value): self
    {
        return $this->arithmetic($path, 'Multiply', $value);
    }

    public function divide(string $path, mixed $value): self
    {
        return $this->arithmetic($path, 'Divide', $value);
    }

    public function condition(string $expression): self
    {
        $copy = clone $this;
        $copy->condition = $expression;
        return $copy;
    }

    public function create(bool $enabled = true): self
    {
        $copy = clone $this;
        $copy->create = $enabled;
        return $copy;
    }

    public function execute(): string
    {
        return $this->index->executeUpdate(
            $this->id,
            $this->operations,
            $this->condition,
            $this->create,
        );
    }

    /** @param array<string,mixed> $payload */
    private function operation(string $path, string $type, array $payload): self
    {
        $copy = clone $this;
        $copy->operations[] = [
            'target' => ['path' => $path],
            'operation_type' => $type,
            'operation' => $payload,
        ];
        return $copy;
    }

    private function arithmetic(string $path, string $operator, mixed $value): self
    {
        return $this->operation($path, 'ArithmeticUpdate', [
            'operator' => $operator,
            'operand' => Protocol::value($value),
        ]);
    }
}
