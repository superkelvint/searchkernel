<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class G
{
    public static function field(string $field): GroupingExpression
    {
        self::name($field, 'grouping field');
        return new GroupingExpression([
            'expr_type' => 'GroupingAttributeExpression',
            'expr' => ['field' => $field],
        ]);
    }

    public static function value(mixed $value): GroupingExpression
    {
        return new GroupingExpression([
            'expr_type' => 'GroupingLiteralExpression',
            'expr' => ['value' => Protocol::value($value)],
        ]);
    }

    public static function fn(string $name, mixed ...$arguments): GroupingExpression
    {
        self::name($name, 'grouping function');
        return new GroupingExpression([
            'expr_type' => 'GroupingFunctionExpression',
            'expr' => [
                'name' => $name,
                'arguments' => array_map(
                    static fn (mixed $argument): array => self::expression($argument)->model,
                    $arguments,
                ),
            ],
        ]);
    }

    public static function aliasRef(string $name): GroupingExpression
    {
        self::name($name, 'grouping alias');
        return new GroupingExpression([
            'expr_type' => 'GroupingAliasReference',
            'expr' => ['name' => $name],
        ]);
    }

    public static function member(GroupingExpression $base, string $member): GroupingExpression
    {
        self::name($member, 'grouping member');
        return new GroupingExpression([
            'expr_type' => 'GroupingMemberExpression',
            'expr' => ['base' => $base->model, 'member' => $member],
        ]);
    }

    public static function lookup(GroupingExpression $base, mixed $key): GroupingExpression
    {
        return new GroupingExpression([
            'expr_type' => 'GroupingMapLookupExpression',
            'expr' => [
                'base' => $base->model,
                'key' => self::expression($key)->model,
            ],
        ]);
    }

    public static function bucket(
        mixed $lower = null,
        mixed $upper = null,
        bool $includeLower = true,
        bool $includeUpper = false,
    ): GroupingExpression {
        return new GroupingExpression([
            'expr_type' => 'GroupingBucketExpression',
            'expr' => [
                'lower' => self::bound($lower),
                'upper' => self::bound($upper),
                'include_lower' => ['value' => $includeLower],
                'include_upper' => ['value' => $includeUpper],
            ],
        ]);
    }

    private static function expression(mixed $value): GroupingExpression
    {
        return $value instanceof GroupingExpression ? $value : self::value($value);
    }

    /** @return array<string,mixed> */
    private static function bound(mixed $value): array
    {
        if ($value === null) {
            return ['bound_type' => 'UnboundedRangeBound', 'bound' => []];
        }
        return [
            'bound_type' => 'FiniteRangeBound',
            'bound' => ['value' => Protocol::value($value)],
        ];
    }

    private static function name(string $value, string $label): void
    {
        if ($value === '') {
            throw new InvalidArgumentException($label . ' must not be empty');
        }
    }
}
