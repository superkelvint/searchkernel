<?php

declare(strict_types=1);

namespace SearchKernel;

final class GroupingExpression
{
    /** @param array<string,mixed> $model */
    public function __construct(public readonly array $model) {}

    public function as(string $label): GroupingOutput
    {
        return new GroupingOutput($this, $label);
    }

    public function plus(mixed $right): self
    {
        return $this->binary('Add', $right);
    }

    public function minus(mixed $right): self
    {
        return $this->binary('Subtract', $right);
    }

    public function multiply(mixed $right): self
    {
        return $this->binary('Multiply', $right);
    }

    public function divide(mixed $right): self
    {
        return $this->binary('Divide', $right);
    }

    public function modulo(mixed $right): self
    {
        return $this->binary('Modulo', $right);
    }

    public function negate(): self
    {
        return new self([
            'expr_type' => 'GroupingUnaryExpression',
            'expr' => ['op' => 'Negative', 'expression' => $this->model],
        ]);
    }

    private function binary(string $operator, mixed $right): self
    {
        $rightExpression = $right instanceof self ? $right : G::value($right);
        return new self([
            'expr_type' => 'GroupingBinaryExpression',
            'expr' => [
                'op' => $operator,
                'left' => $this->model,
                'right' => $rightExpression->model,
            ],
        ]);
    }
}
