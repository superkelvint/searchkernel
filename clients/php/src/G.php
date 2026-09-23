<?php

declare(strict_types=1);

namespace SearchKernel;

final class G
{
    public static function field(string $field): GroupingExpression
    {
        return new GroupingExpression([
            'expr_type' => 'GroupingAttributeExpression',
            'expr' => ['field' => $field],
        ]);
    }

    public static function aliasRef(string $name): GroupingExpression
    {
        return new GroupingExpression([
            'expr_type' => 'GroupingAliasReference',
            'expr' => ['name' => $name],
        ]);
    }
}
