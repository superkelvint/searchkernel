<?php

declare(strict_types=1);

namespace SearchKernel;

final class MatchedUpdate
{
    /** @param array<string,mixed> $wire */
    private function __construct(private readonly array $wire) {}

    public static function assign(mixed $value): self
    {
        return new self([
            'operation_type' => 'AssignUpdate',
            'operation' => ['value' => Protocol::value($value)],
        ]);
    }

    public static function clear(): self
    {
        return new self([
            'operation_type' => 'ClearUpdate',
            'operation' => [],
        ]);
    }

    public static function increment(mixed $value): self
    {
        return self::arithmetic('Increment', $value);
    }

    public static function decrement(mixed $value): self
    {
        return self::arithmetic('Decrement', $value);
    }

    public static function multiply(mixed $value): self
    {
        return self::arithmetic('Multiply', $value);
    }

    public static function divide(mixed $value): self
    {
        return self::arithmetic('Divide', $value);
    }

    /** @return array<string,mixed> */
    public function toWire(): array
    {
        return $this->wire;
    }

    private static function arithmetic(string $operator, mixed $value): self
    {
        return new self([
            'operation_type' => 'ArithmeticUpdate',
            'operation' => [
                'operator' => $operator,
                'operand' => Protocol::value($value),
            ],
        ]);
    }
}
