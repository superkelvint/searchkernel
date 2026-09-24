<?php

declare(strict_types=1);

namespace SearchKernel;

final class GroupingOrder
{
    public function __construct(public readonly GroupingExpression $expression) {}

    public static function asc(GroupingExpression $expression): self
    {
        return new self(new GroupingExpression([
            'expr_type' => 'GroupingUnaryExpression',
            'expr' => ['op' => 'Positive', 'expression' => $expression->model],
        ]));
    }

    public static function desc(GroupingExpression $expression): self
    {
        return new self(new GroupingExpression([
            'expr_type' => 'GroupingUnaryExpression',
            'expr' => ['op' => 'Negative', 'expression' => $expression->model],
        ]));
    }
}
