<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Query
{
    /** @return array<string,mixed> */
    public static function term(mixed $value): array
    {
        return self::typed(self::termExpr($value));
    }

    /** @return array<string,mixed> */
    public static function contains(string $field, mixed $item): array
    {
        self::name($field, 'query field');
        return self::typed([
            'expr_type' => 'QueryContains',
            'expr' => [
                'field' => ['path' => $field],
                'item' => self::itemExpr($item),
            ],
        ]);
    }

    /** @param array<string,mixed> ...$queries
     *  @return array<string,mixed>
     */
    public static function allOf(array ...$queries): array
    {
        return self::clauses('QueryAnd', $queries);
    }

    /** @param array<string,mixed> ...$queries
     *  @return array<string,mixed>
     */
    public static function anyOf(array ...$queries): array
    {
        return self::clauses('QueryOr', $queries);
    }

    /** @param array<string,mixed> $query
     *  @return array<string,mixed>
     */
    public static function negate(array $query): array
    {
        return self::typed([
            'expr_type' => 'QueryNot',
            'expr' => ['clause' => self::expr($query)],
        ]);
    }

    /** @return array<string,mixed> */
    public static function alwaysTrue(): array
    {
        return self::typed(['expr_type' => 'QueryTrue', 'expr' => []]);
    }

    /** @return array<string,mixed> */
    public static function alwaysFalse(): array
    {
        return self::typed(['expr_type' => 'QueryFalse', 'expr' => []]);
    }

    /** @return array<string,mixed> */
    public static function eq(string $field, mixed $value): array
    {
        return self::comparison($field, 'Eq', $value);
    }

    /** @return array<string,mixed> */
    public static function ne(string $field, mixed $value): array
    {
        return self::comparison($field, 'Ne', $value);
    }

    /** @return array<string,mixed> */
    public static function lt(string $field, mixed $value): array
    {
        return self::comparison($field, 'Lt', $value);
    }

    /** @return array<string,mixed> */
    public static function lte(string $field, mixed $value): array
    {
        return self::comparison($field, 'Lte', $value);
    }

    /** @return array<string,mixed> */
    public static function gt(string $field, mixed $value): array
    {
        return self::comparison($field, 'Gt', $value);
    }

    /** @return array<string,mixed> */
    public static function gte(string $field, mixed $value): array
    {
        return self::comparison($field, 'Gte', $value);
    }

    /**
     * Range convenience matching the public gte/gt/lte/lt vocabulary.
     * At most one lower and one upper bound may be supplied.
     *
     * @return array<string,mixed>
     */
    public static function range(
        string $field,
        mixed $gte = null,
        mixed $gt = null,
        mixed $lte = null,
        mixed $lt = null,
    ): array {
        self::name($field, 'range field');
        if ($gte !== null && $gt !== null) {
            throw new InvalidArgumentException('range accepts at most one of gte and gt');
        }
        if ($lte !== null && $lt !== null) {
            throw new InvalidArgumentException('range accepts at most one of lte and lt');
        }

        $payload = [
            'field' => ['path' => $field],
            'lower' => self::rangeBound($gte ?? $gt),
            'upper' => self::rangeBound($lte ?? $lt),
        ];
        if ($gte !== null || $gt !== null) {
            $payload['include_lower'] = ['value' => $gte !== null];
        }
        if ($lte !== null || $lt !== null) {
            $payload['include_upper'] = ['value' => $lte !== null];
        }
        return self::typed(['expr_type' => 'QueryRange', 'expr' => $payload]);
    }

    /** @param list<mixed> $values
     *  @return array<string,mixed>
     */
    public static function inValues(string $field, array $values): array
    {
        self::name($field, 'IN field');
        if ($values === []) {
            throw new InvalidArgumentException('IN query requires at least one value');
        }
        return self::typed([
            'expr_type' => 'QueryIn',
            'expr' => [
                'field' => ['path' => $field],
                'values' => array_map(Protocol::value(...), array_values($values)),
            ],
        ]);
    }

    /** @param mixed ...$items
     *  @return array<string,mixed>
     */
    public static function phrase(mixed ...$items): array
    {
        return self::terms('QueryPhrase', $items);
    }

    /** @param mixed ...$items
     *  @return array<string,mixed>
     */
    public static function near(mixed ...$items): array
    {
        return self::terms('QueryNear', $items);
    }

    /** @param mixed ...$items
     *  @return array<string,mixed>
     */
    public static function onear(mixed ...$items): array
    {
        return self::terms('QueryONear', $items);
    }

    /** @param array<string,mixed> ...$queries
     *  @return array<string,mixed>
     */
    public static function sameElement(array ...$queries): array
    {
        return self::clauses('QuerySameElement', $queries);
    }

    /** @param array<string,mixed> ...$queries
     *  @return array<string,mixed>
     */
    public static function equiv(array ...$queries): array
    {
        return self::clauses('QueryEquiv', $queries);
    }

    /** @return array<string,mixed> */
    public static function fuzzy(mixed $value): array
    {
        return self::typed([
            'expr_type' => 'QueryFuzzy',
            'expr' => ['value' => Protocol::value($value)],
        ]);
    }

    /** @return array<string,mixed> */
    public static function matches(string $field, string $pattern): array
    {
        self::name($field, 'matches field');
        return self::typed([
            'expr_type' => 'QueryMatches',
            'expr' => ['field' => ['path' => $field], 'pattern' => $pattern],
        ]);
    }

    /** @return array<string,mixed> */
    public static function text(string $text): array
    {
        return self::typed(['expr_type' => 'QueryText', 'expr' => ['text' => $text]]);
    }

    /** @return array<string,mixed> */
    public static function userInput(string $value): array
    {
        return self::typed(['expr_type' => 'QueryUserInput', 'expr' => ['value' => $value]]);
    }

    /** @return array<string,mixed> */
    public static function userQuery(): array
    {
        return self::typed(['expr_type' => 'QueryUserQuery', 'expr' => []]);
    }

    /** @param array<int|string,int> $items
     *  @return array<string,mixed>
     */
    public static function dotProduct(string $field, array $items): array
    {
        return self::weighted('QueryDotProduct', $field, $items);
    }

    /** @param array<int|string,int> $items
     *  @return array<string,mixed>
     */
    public static function weightedSet(string $field, array $items): array
    {
        return self::weighted('QueryWeightedSet', $field, $items);
    }

    /** @param array<int|string,int> $items
     *  @return array<string,mixed>
     */
    public static function wand(string $field, array $items): array
    {
        return self::weighted('QueryWand', $field, $items);
    }

    /** @param array<string,mixed> ...$queries
     *  @return array<string,mixed>
     */
    public static function weakAnd(array ...$queries): array
    {
        return self::clauses('QueryWeakAnd', $queries);
    }

    /** @param array<string,mixed> $match
     *  @param array<string,mixed> ...$ranking
     *  @return array<string,mixed>
     */
    public static function rank(array $match, array ...$ranking): array
    {
        return self::typed([
            'expr_type' => 'QueryRank',
            'expr' => [
                'match' => self::expr($match),
                'rank' => array_map([self::class, 'expr'], $ranking),
            ],
        ]);
    }

    /** @param array<string,mixed> $query
     *  @return array<string,mixed>
     */
    public static function labeled(array $query, string $label, float $score): array
    {
        self::name($label, 'query label');
        if (!is_finite($score)) {
            throw new InvalidArgumentException('query label score must be finite');
        }
        return self::typed([
            'expr_type' => 'QueryLabeled',
            'expr' => ['query' => self::expr($query), 'label' => $label, 'score' => $score],
        ]);
    }

    /** @param array<string,mixed> $query
     *  @return array<string,mixed>
     */
    public static function nonEmpty(array $query): array
    {
        return self::typed([
            'expr_type' => 'QueryNonEmpty',
            'expr' => ['query' => self::expr($query)],
        ]);
    }

    /** @return array<string,mixed> */
    public static function geoLocation(
        string $field,
        float $lat,
        float $lon,
        string $radius,
    ): array {
        self::name($field, 'geo field');
        self::name($radius, 'geo radius');
        if (!is_finite($lat) || !is_finite($lon)) {
            throw new InvalidArgumentException('geo coordinates must be finite');
        }
        return self::typed([
            'expr_type' => 'QueryGeoLocation',
            'expr' => [
                'field' => ['path' => $field],
                'latitude' => $lat,
                'longitude' => $lon,
                'radius' => $radius,
            ],
        ]);
    }

    /** @return array<string,mixed> */
    public static function geoBox(
        string $field,
        float $south,
        float $west,
        float $north,
        float $east,
    ): array {
        self::name($field, 'geo field');
        foreach ([$south, $west, $north, $east] as $coordinate) {
            if (!is_finite($coordinate)) {
                throw new InvalidArgumentException('geo coordinates must be finite');
            }
        }
        return self::typed([
            'expr_type' => 'QueryGeoBoundingBox',
            'expr' => [
                'field' => ['path' => $field],
                'south' => $south,
                'west' => $west,
                'north' => $north,
                'east' => $east,
            ],
        ]);
    }

    /** @param array<string,mixed> $annotations
     *  @return array<string,mixed>
     */
    public static function nearestNeighbor(
        string $documentField,
        string $queryTensor,
        array $annotations = [],
    ): array {
        self::name($documentField, 'nearest-neighbor document field');
        self::name($queryTensor, 'nearest-neighbor query tensor');
        $query = self::typed([
            'expr_type' => 'QueryNearestNeighbor',
            'expr' => [
                'document_field' => ['path' => $documentField],
                'query_tensor' => $queryTensor,
            ],
        ]);
        return $annotations === [] ? $query : self::annotate($query, $annotations);
    }

    /** @param array<string,mixed> $features
     *  @param array<string,mixed> $rangeFeatures
     *  @return array<string,mixed>
     */
    public static function predicate(
        string $field,
        array $features = [],
        array $rangeFeatures = [],
    ): array {
        self::name($field, 'predicate field');
        return self::typed([
            'expr_type' => 'QueryPredicate',
            'expr' => [
                'field' => ['path' => $field],
                'features' => self::valueObject($features),
                'range_features' => self::valueObject($rangeFeatures),
            ],
        ]);
    }

    /** @param array<string,mixed> $query
     *  @param array<string,mixed> $annotations
     *  @return array<string,mixed>
     */
    public static function annotate(array $query, array $annotations): array
    {
        $expr = self::expr($query);
        $existing = [];
        foreach ($expr['annotations'] ?? [] as $entry) {
            if (!is_array($entry) || !is_string($entry['key'] ?? null)) {
                throw new InvalidArgumentException('malformed existing query annotation');
            }
            $existing[$entry['key']] = $entry;
        }
        foreach (Protocol::valueEntries($annotations) as $entry) {
            if (array_key_exists($entry['key'], $existing)) {
                throw new InvalidArgumentException('duplicate query annotation: ' . $entry['key']);
            }
            $existing[$entry['key']] = $entry;
        }
        ksort($existing, SORT_STRING);
        $expr['annotations'] = array_values($existing);
        return self::typed($expr);
    }

    /** @return array<string,mixed> */
    private static function comparison(string $field, string $op, mixed $value): array
    {
        self::name($field, 'comparison field');
        return self::typed([
            'expr_type' => 'QueryComparison',
            'expr' => [
                'field' => ['path' => $field],
                'op' => $op,
                'value' => Protocol::value($value),
            ],
        ]);
    }

    /** @return array<string,mixed> */
    private static function rangeBound(mixed $value): array
    {
        if ($value === null) {
            return ['bound_type' => 'UnboundedRangeBound', 'bound' => []];
        }
        return [
            'bound_type' => 'FiniteRangeBound',
            'bound' => ['value' => Protocol::value($value)],
        ];
    }

    /** @param mixed ...$items
     *  @return array<string,mixed>
     */
    private static function terms(string $type, array $items): array
    {
        if ($items === []) {
            throw new InvalidArgumentException($type . ' requires at least one item');
        }
        return self::typed([
            'expr_type' => $type,
            'expr' => ['terms' => array_map([self::class, 'itemExpr'], $items)],
        ]);
    }

    /** @param list<array<string,mixed>> $queries
     *  @return array<string,mixed>
     */
    private static function clauses(string $type, array $queries): array
    {
        if ($queries === []) {
            throw new InvalidArgumentException('query operator requires at least one item');
        }
        return self::typed([
            'expr_type' => $type,
            'expr' => ['clauses' => array_map([self::class, 'expr'], $queries)],
        ]);
    }

    /** @param array<int|string,int> $items
     *  @return array<string,mixed>
     */
    private static function weighted(string $type, string $field, array $items): array
    {
        self::name($field, 'weighted-query field');
        if ($items === []) {
            throw new InvalidArgumentException($type . ' requires at least one weighted item');
        }
        $weighted = [];
        foreach ($items as $key => $weight) {
            if (!is_int($weight)) {
                throw new InvalidArgumentException('weighted query weights must be integers');
            }
            $weighted[] = ['key' => Protocol::value($key), 'weight' => $weight];
        }
        usort(
            $weighted,
            static fn (array $a, array $b): int =>
                JsonEncoder::encode($a['key']) <=> JsonEncoder::encode($b['key']),
        );
        return self::typed([
            'expr_type' => $type,
            'expr' => ['field' => ['path' => $field], 'items' => $weighted],
        ]);
    }

    /** @param array<string,mixed> $values
     *  @return array<string,mixed>
     */
    private static function valueObject(array $values): array
    {
        $encoded = Protocol::value(new ValueObject($values));
        if (($encoded['kind_type'] ?? null) !== 'ValueObject' || !is_array($encoded['kind'] ?? null)) {
            throw new InvalidArgumentException('predicate feature maps must be portable objects');
        }
        return $encoded['kind'];
    }

    /** @return array<string,mixed> */
    private static function termExpr(mixed $value): array
    {
        return [
            'expr_type' => 'TermQuery',
            'expr' => ['value' => Protocol::value($value)],
        ];
    }

    /** @return array<string,mixed> */
    private static function itemExpr(mixed $value): array
    {
        if (is_array($value) && ($value['source_type'] ?? null) === 'TypedQuery') {
            return self::expr($value);
        }
        return self::termExpr($value);
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

    private static function name(string $value, string $label): void
    {
        if ($value === '') {
            throw new InvalidArgumentException($label . ' must not be empty');
        }
    }
}
