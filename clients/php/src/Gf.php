<?php

declare(strict_types=1);

namespace SearchKernel;

final class Gf
{
    /** @return array<string,mixed> */
    public static function regex(GroupingExpression $expression, string $pattern): array
    {
        return [
            'expr_type' => 'GroupingFilterRegex',
            'expr' => [
                'expression' => $expression->model,
                'pattern' => $pattern,
            ],
        ];
    }
}
