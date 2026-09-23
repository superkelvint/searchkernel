<?php

declare(strict_types=1);

namespace SearchKernel;

final class Agg
{
    public static function count(): GroupingExpression
    {
        return self::aggregation('CountAggregation', []);
    }

    public static function sum(string $field): GroupingExpression
    {
        return self::fieldAggregation('SumAggregation', $field);
    }

    public static function avg(string $field): GroupingExpression
    {
        return self::fieldAggregation('AvgAggregation', $field);
    }

    public static function min(string $field): GroupingExpression
    {
        return self::fieldAggregation('MinAggregation', $field);
    }

    public static function max(string $field): GroupingExpression
    {
        return self::fieldAggregation('MaxAggregation', $field);
    }

    public static function stddev(string $field): GroupingExpression
    {
        return self::fieldAggregation('StdDevAggregation', $field);
    }

    public static function summary(string $summary = ''): GroupingExpression
    {
        return self::aggregation('SummaryAggregation', ['summary' => $summary]);
    }

    private static function fieldAggregation(string $type, string $field): GroupingExpression
    {
        return self::aggregation($type, ['expression' => G::field($field)->model]);
    }

    /** @param array<string,mixed> $payload */
    private static function aggregation(string $type, array $payload): GroupingExpression
    {
        return new GroupingExpression([
            'expr_type' => 'GroupingAggregationExpression',
            'expr' => [
                'aggregation_type' => $type,
                'aggregation' => $payload,
            ],
        ]);
    }
}
