<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Query
{
    /** @return array<string,mixed> */
    public static function contains(string $field, mixed $value): array
    {
        return self::typed([
            'expr_type' => 'QueryContains',
            'expr' => [
                'field' => ['path' => $field],
                'item' => self::term($value),
            ],
        ]);
    }

    /** @param array<string,mixed> ...$queries
     *  @return array<string,mixed>
     */
    public static function anyOf(array ...$queries): array
    {
        if ($queries === []) {
            throw new InvalidArgumentException('query operator requires at least one item');
        }
        return self::typed([
            'expr_type' => 'QueryOr',
            'expr' => [
                'clauses' => array_map([self::class, 'expr'], $queries),
            ],
        ]);
    }

    /** @return array<string,mixed> */
    public static function nearestNeighbor(string $documentField, string $queryTensor): array
    {
        return self::typed([
            'expr_type' => 'QueryNearestNeighbor',
            'expr' => [
                'document_field' => ['path' => $documentField],
                'query_tensor' => $queryTensor,
            ],
        ]);
    }

    /** @return array<string,mixed> */
    private static function term(mixed $value): array
    {
        return [
            'expr_type' => 'TermQuery',
            'expr' => ['value' => Protocol::value($value)],
        ];
    }

    /** @param array<string,mixed> $expr
     *  @return array<string,mixed>
     */
    private static function typed(array $expr): array
    {
        return [
            'source_type' => 'TypedQuery',
            'source' => ['query' => $expr],
        ];
    }

    /** @param array<string,mixed> $source
     *  @return array<string,mixed>
     */
    private static function expr(array $source): array
    {
        if (($source['source_type'] ?? null) !== 'TypedQuery' || !is_array($source['source']['query'] ?? null)) {
            throw new InvalidArgumentException('typed query clauses must come from Query helpers');
        }
        return $source['source']['query'];
    }
}
