<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Gf
{
    /** @return array<string,mixed> */
    public static function regex(GroupingExpression $expression, string $pattern): array
    {
        return self::filter('GroupingFilterRegex', [
            'expression' => $expression->model,
            'pattern' => $pattern,
        ]);
    }

    /** @return array<string,mixed> */
    public static function range(
        GroupingExpression $expression,
        mixed $lower,
        mixed $upper,
        bool $includeLower = true,
        bool $includeUpper = false,
    ): array {
        return self::filter('GroupingFilterRange', [
            'expression' => $expression->model,
            'lower' => self::expression($lower)->model,
            'upper' => self::expression($upper)->model,
            'include_lower' => ['value' => $includeLower],
            'include_upper' => ['value' => $includeUpper],
        ]);
    }

    /** @return array<string,mixed> */
    public static function isTrue(GroupingExpression $expression): array
    {
        return self::filter('GroupingFilterIsTrue', ['expression' => $expression->model]);
    }

    /** @param array<string,mixed> ...$clauses
     *  @return array<string,mixed>
     */
    public static function allOf(array ...$clauses): array
    {
        return self::clauses('GroupingFilterAnd', $clauses);
    }

    /** @param array<string,mixed> ...$clauses
     *  @return array<string,mixed>
     */
    public static function anyOf(array ...$clauses): array
    {
        return self::clauses('GroupingFilterOr', $clauses);
    }

    /** @param array<string,mixed> $clause
     *  @return array<string,mixed>
     */
    public static function negate(array $clause): array
    {
        return self::filter('GroupingFilterNot', ['clause' => self::validate($clause)]);
    }

    /** @param list<array<string,mixed>> $clauses
     *  @return array<string,mixed>
     */
    private static function clauses(string $type, array $clauses): array
    {
        if ($clauses === []) {
            throw new InvalidArgumentException($type . ' requires at least one clause');
        }
        return self::filter($type, [
            'clauses' => array_map([self::class, 'validate'], $clauses),
        ]);
    }

    /** @param array<string,mixed> $payload
     *  @return array<string,mixed>
     */
    private static function filter(string $type, array $payload): array
    {
        return ['expr_type' => $type, 'expr' => $payload];
    }

    /** @param array<string,mixed> $filter
     *  @return array<string,mixed>
     */
    private static function validate(array $filter): array
    {
        if (!is_string($filter['expr_type'] ?? null) || !is_array($filter['expr'] ?? null)) {
            throw new InvalidArgumentException('grouping filter must come from Gf helpers');
        }
        return $filter;
    }

    private static function expression(mixed $value): GroupingExpression
    {
        return $value instanceof GroupingExpression ? $value : G::value($value);
    }
}
