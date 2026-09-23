<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Group
{
    private ?int $limit = null;
    private ?int $precision = null;
    /** @var list<GroupingOrder> */
    private array $orders = [];
    /** @var list<GroupingOutput> */
    private array $outputs = [];
    /** @var list<Group> */
    private array $subgroups = [];
    /** @var list<array{0:string,1:GroupingExpression}> */
    private array $aliases = [];
    /** @var array<string,mixed>|null */
    private ?array $keep = null;

    private function __construct(private readonly GroupingExpression $expression) {}

    public static function by(string|GroupingExpression $field): self
    {
        return new self(is_string($field) ? G::field($field) : $field);
    }

    public function limit(int $value): self
    {
        self::uint32($value, 'group limit');
        $copy = clone $this;
        $copy->limit = $value;
        return $copy;
    }

    public function precision(int $value): self
    {
        self::uint32($value, 'group precision');
        $copy = clone $this;
        $copy->precision = $value;
        return $copy;
    }

    public function orderBy(GroupingOrder ...$orders): self
    {
        $copy = clone $this;
        array_push($copy->orders, ...$orders);
        return $copy;
    }

    public function output(GroupingExpression|GroupingOutput ...$outputs): self
    {
        $copy = clone $this;
        foreach ($outputs as $output) {
            $copy->outputs[] = $output instanceof GroupingOutput ? $output : $output->as('');
        }
        return $copy;
    }

    public function subgroup(self $group): self
    {
        $copy = clone $this;
        $copy->subgroups[] = $group;
        return $copy;
    }

    public function alias(string $name, GroupingExpression $expression): self
    {
        if ($name === '') {
            throw new InvalidArgumentException('group alias must not be empty');
        }
        $copy = clone $this;
        $copy->aliases[] = [$name, $expression];
        return $copy;
    }

    /** @param array<string,mixed> $filter */
    public function keep(array $filter): self
    {
        $copy = clone $this;
        $copy->keep = $filter;
        return $copy;
    }

    /** @return array<string,mixed> */
    public function toRequest(): array
    {
        return [
            'source_type' => 'StructuredGroupingSource',
            'source' => ['root' => ['operations' => $this->operations()]],
        ];
    }

    /** @return list<array<string,mixed>> */
    private function operations(): array
    {
        return [
            [
                'operation_type' => 'GroupingGroupOperation',
                'operation' => ['expression' => $this->expression->model],
            ],
            [
                'operation_type' => 'GroupingEachOperation',
                'operation' => ['operations' => $this->bodyOperations()],
            ],
        ];
    }

    /** @return list<array<string,mixed>> */
    private function bodyOperations(): array
    {
        $operations = [];
        foreach ($this->aliases as [$name, $expression]) {
            $operations[] = [
                'operation_type' => 'GroupingAliasOperation',
                'operation' => ['name' => $name, 'expression' => $expression->model],
            ];
        }
        if ($this->limit !== null) {
            $operations[] = [
                'operation_type' => 'GroupingMaxOperation',
                'operation' => [
                    'value_type' => 'FiniteGroupingMax',
                    'value' => ['value' => $this->limit],
                ],
            ];
        }
        if ($this->precision !== null) {
            $operations[] = [
                'operation_type' => 'GroupingPrecisionOperation',
                'operation' => ['value' => $this->precision],
            ];
        }
        if ($this->orders !== []) {
            $operations[] = [
                'operation_type' => 'GroupingOrderOperation',
                'operation' => [
                    'expressions' => array_map(
                        static fn (GroupingOrder $order): array => ['expression' => $order->expression->model],
                        $this->orders,
                    ),
                ],
            ];
        }
        if ($this->keep !== null) {
            $operations[] = [
                'operation_type' => 'GroupingKeepOperation',
                'operation' => ['filter' => $this->keep],
            ];
        }
        if ($this->outputs !== []) {
            $operations[] = [
                'operation_type' => 'GroupingOutputOperation',
                'operation' => [
                    'outputs' => array_map(
                        static fn (GroupingOutput $output): array => $output->toWire(),
                        $this->outputs,
                    ),
                ],
            ];
        }
        foreach ($this->subgroups as $subgroup) {
            $operations[] = [
                'operation_type' => 'GroupingAllOperation',
                'operation' => ['operations' => $subgroup->operations()],
            ];
        }
        return $operations;
    }

    /** @return array<string,mixed> */
    public static function facetRequest(string $field, string $name, int $limit, string $sort): array
    {
        self::uint32($limit, 'facet limit');
        $order = match ($sort) {
            'count_desc' => GroupingOrder::desc(Agg::count()),
            'count_asc' => GroupingOrder::asc(Agg::count()),
            'value_desc' => GroupingOrder::desc(G::field($field)),
            'value_asc' => GroupingOrder::asc(G::field($field)),
            default => throw new InvalidArgumentException('facet sort must be count_desc, count_asc, value_asc, or value_desc'),
        };
        $group = self::by($field)->limit($limit)->orderBy($order)->output(Agg::count());
        $request = $group->toRequest();
        $request['source']['root']['label'] = $name;
        return $request;
    }

    /** @param list<GroupingOutput> $outputs
     *  @return array<string,mixed>
     */
    public static function rootAggregationRequest(array $outputs): array
    {
        return [
            'source_type' => 'StructuredGroupingSource',
            'source' => [
                'root' => [
                    'operations' => [[
                        'operation_type' => 'GroupingOutputOperation',
                        'operation' => [
                            'outputs' => array_map(
                                static fn (GroupingOutput $output): array => $output->toWire(),
                                $outputs,
                            ),
                        ],
                    ]],
                ],
            ],
        ];
    }

    private static function uint32(int $value, string $label): void
    {
        if ($value < 0 || $value > 0xFFFF_FFFF) {
            throw new InvalidArgumentException($label . ' must be a uint32');
        }
    }
}
