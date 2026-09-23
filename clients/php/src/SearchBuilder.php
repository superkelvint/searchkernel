<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

class SearchBuilder
{
    /** @var list<array{source:string,bindings:array<string,mixed>}> */
    private array $filters = [];
    /** @var list<array<string,mixed>> */
    private array $sorts = [];
    /** @var list<array<string,mixed>> */
    private array $searchFields = [];
    /** @var list<Group> */
    private array $groups = [];
    /** @var list<array{field:string,name:string,limit:int,sort:string}> */
    private array $facets = [];
    /** @var list<GroupingOutput> */
    private array $aggregates = [];
    /** @var list<string>|null */
    private ?array $fields = null;
    /** @var list<string>|null */
    private ?array $highlights = null;
    /** @var array<string,mixed> */
    private array $inputs = [];
    /** @var array<string,string> */
    private array $rankProperties = [];
    /** @var array<string,mixed> */
    private array $properties = [];
    private ?string $summary = null;
    private ?string $ranking = null;
    private ?int $limit = null;
    private ?int $offset = null;
    private ?int $timeoutMs = null;
    private bool $paged = false;

    /** @param array<string,mixed> $query */
    public function __construct(
        private readonly Index $index,
        private readonly array $query,
    ) {}

    public function searchField(string $field, ?float $weight = null): self
    {
        if ($field === '') {
            throw new InvalidArgumentException('search field must not be empty');
        }
        foreach ($this->searchFields as $existing) {
            if (($existing['field'] ?? null) === $field) {
                throw new InvalidArgumentException('duplicate search field: ' . $field);
            }
        }

        $entry = ['field' => $field];
        if ($weight !== null) {
            if (!is_finite($weight)) {
                throw new InvalidArgumentException('search field weight must be finite');
            }
            $entry['weight'] = ['value' => $weight];
        }

        $copy = clone $this;
        $copy->searchFields[] = $entry;
        return $copy;
    }

    public function searchFields(string ...$fields): self
    {
        $copy = $this;
        foreach ($fields as $field) {
            $copy = $copy->searchField($field);
        }
        return $copy;
    }

    /** @param array<string,mixed> $bindings */
    public function filter(string $source, array $bindings = []): self
    {
        if (trim($source) === '') {
            throw new InvalidArgumentException('filter source must not be empty');
        }
        $copy = clone $this;
        $copy->filters[] = ['source' => $source, 'bindings' => $bindings];
        return $copy;
    }

    public function sort(string $field, string $direction = 'asc', mixed $missing = null): self
    {
        if ($field === '') {
            throw new InvalidArgumentException('sort field must not be empty');
        }
        $sort = [
            'target' => [
                'target_type' => 'FieldSortTarget',
                'target' => ['field' => $field],
            ],
            'direction' => self::direction($direction),
        ];
        if ($missing !== null) {
            $sort['missing'] = self::missingPolicy($missing);
        }
        return $this->withSort($sort);
    }

    public function sortRelevance(string $direction = 'desc'): self
    {
        return $this->withSort([
            'target' => [
                'target_type' => 'RelevanceSortTarget',
                'target' => [],
            ],
            'direction' => self::direction($direction),
        ]);
    }

    public function sortFeature(string $feature, string $direction = 'desc'): self
    {
        if ($feature === '') {
            throw new InvalidArgumentException('rank feature must not be empty');
        }
        return $this->withSort([
            'target' => [
                'target_type' => 'RankFeatureSortTarget',
                'target' => ['feature' => $feature],
            ],
            'direction' => self::direction($direction),
        ]);
    }

    public function rawSort(string $expression, string $direction = 'asc'): self
    {
        if (trim($expression) === '') {
            throw new InvalidArgumentException('raw sort expression must not be empty');
        }
        return $this->withSort([
            'target' => [
                'target_type' => 'RawSortTarget',
                'target' => ['expression' => $expression],
            ],
            'direction' => self::direction($direction),
        ]);
    }

    public function fields(string ...$fields): self
    {
        $copy = clone $this;
        $copy->fields = $fields;
        return $copy;
    }

    public function summary(string $name): self
    {
        $copy = clone $this;
        $copy->summary = $name;
        return $copy;
    }

    public function highlight(string ...$fields): self
    {
        $copy = clone $this;
        $copy->highlights = $fields;
        return $copy;
    }

    public function facet(
        string $field,
        ?string $name = null,
        int $limit = 10,
        string $sort = 'count_desc',
    ): self {
        if ($field === '') {
            throw new InvalidArgumentException('facet field must not be empty');
        }
        self::uint32($limit, 'facet limit');
        $copy = clone $this;
        $copy->facets[] = [
            'field' => $field,
            'name' => $name ?? $field,
            'limit' => $limit,
            'sort' => $sort,
        ];
        return $copy;
    }

    public function group(Group $group): self
    {
        $copy = clone $this;
        $copy->groups[] = $group;
        return $copy;
    }

    public function aggregate(GroupingExpression|GroupingOutput $expression): self
    {
        $copy = clone $this;
        $copy->aggregates[] = $expression instanceof GroupingOutput
            ? $expression
            : $expression->as('');
        return $copy;
    }

    public function ranking(string $profile): self
    {
        if ($profile === '') {
            throw new InvalidArgumentException('ranking profile must not be empty');
        }
        $copy = clone $this;
        $copy->ranking = $profile;
        return $copy;
    }

    public function input(string $name, mixed $value): self
    {
        if ($name === '') {
            throw new InvalidArgumentException('query input name must not be empty');
        }
        $copy = clone $this;
        $copy->inputs[$name] = $value;
        return $copy;
    }

    public function rankProperty(string $name, mixed $value): self
    {
        if ($name === '') {
            throw new InvalidArgumentException('rank property name must not be empty');
        }
        $copy = clone $this;
        $copy->rankProperties[$name] = (string) $value;
        return $copy;
    }

    public function property(string $name, mixed $value): self
    {
        if ($name === '') {
            throw new InvalidArgumentException('property name must not be empty');
        }
        $copy = clone $this;
        $copy->properties[$name] = $value;
        return $copy;
    }

    public function limit(int $limit): self
    {
        self::uint32($limit, 'limit');
        if ($this->paged) {
            throw new InvalidArgumentException('limit cannot be combined with page()');
        }
        $copy = clone $this;
        $copy->limit = $limit;
        return $copy;
    }

    public function offset(int $offset): self
    {
        self::uint32($offset, 'offset');
        if ($this->paged) {
            throw new InvalidArgumentException('offset cannot be combined with page()');
        }
        $copy = clone $this;
        $copy->offset = $offset;
        return $copy;
    }

    public function page(int $number, int $size): self
    {
        if ($number < 1 || $this->limit !== null || $this->offset !== null) {
            throw new InvalidArgumentException('invalid page() combination');
        }
        self::uint32($size, 'page size');
        if ($size !== 0 && ($number - 1) > intdiv(0xFFFF_FFFF, $size)) {
            throw new InvalidArgumentException('page range exceeds uint32 range');
        }
        $offset = ($number - 1) * $size;
        $copy = clone $this;
        $copy->paged = true;
        $copy->offset = $offset;
        $copy->limit = $size;
        return $copy;
    }

    public function timeout(int $milliseconds): self
    {
        if ($milliseconds < 0) {
            throw new InvalidArgumentException('timeout must be non-negative');
        }
        $copy = clone $this;
        $copy->timeoutMs = $milliseconds;
        return $copy;
    }

    public function execute(): SearchResult
    {
        $result = $this->index->executeSearchRequest($this->request());
        foreach ($this->facets as $facet) {
            $result->projectFacet($facet['name'], $facet['field']);
        }
        return $result;
    }

    /** @return array<string,mixed> */
    public function inspect(): array
    {
        return $this->index->inspectSearchRequest($this->request());
    }

    /** @return array<string,mixed> */
    public function request(): array
    {
        $request = ['query' => $this->query];

        if ($this->searchFields !== []) {
            $request['search_fields'] = ['fields' => $this->searchFields];
        }

        if ($this->filters !== []) {
            $request['filters'] = [
                'textual' => array_map(
                    static fn (array $clause): array => [
                        'source' => $clause['source'],
                        'bindings' => Protocol::valueEntries($clause['bindings']),
                    ],
                    $this->filters,
                ),
            ];
        }
        if ($this->sorts !== []) {
            $request['sort'] = $this->sorts;
        }
        $grouping = array_map(
            static fn (Group $group): array => $group->toRequest(),
            $this->groups,
        );
        foreach ($this->facets as $facet) {
            $grouping[] = Group::facetRequest(
                $facet['field'],
                $facet['name'],
                $facet['limit'],
                $facet['sort'],
            );
        }
        if ($this->aggregates !== []) {
            $grouping[] = Group::rootAggregationRequest($this->aggregates);
        }
        if ($grouping !== []) {
            $request['grouping'] = $grouping;
        }
        if ($this->ranking !== null || $this->rankProperties !== []) {
            $request['ranking'] = [
                'profile' => $this->ranking ?? 'default',
                'properties' => Protocol::stringEntries($this->rankProperties),
            ];
        }
        if ($this->inputs !== []) {
            $inputs = $this->inputs;
            ksort($inputs, SORT_STRING);
            $request['inputs'] = [];
            foreach ($inputs as $name => $value) {
                $request['inputs'][] = [
                    'name' => $name,
                    'input_type' => 'LiteralQueryInput',
                    'input' => ['value' => Protocol::value($value)],
                ];
            }
        }
        if ($this->limit !== null) {
            $request['limit'] = ['value' => $this->limit];
        }
        if ($this->offset !== null) {
            $request['offset'] = ['value' => $this->offset];
        }
        if ($this->timeoutMs !== null) {
            $request['timeout_ms'] = ['value' => $this->timeoutMs];
        }

        $presentation = [];
        if ($this->summary !== null) {
            $presentation['summary'] = $this->summary;
        }
        if ($this->fields !== null) {
            $presentation['fields'] = ['fields' => $this->fields];
        }
        if ($this->highlights !== null) {
            $presentation['highlighting'] = ['fields' => $this->highlights];
        }
        if ($presentation !== []) {
            $request['presentation'] = $presentation;
        }

        if ($this->properties !== []) {
            $request['properties'] = Protocol::valueEntries($this->properties);
        }

        return $request;
    }

    /** @param array<string,mixed> $sort */
    private function withSort(array $sort): self
    {
        $copy = clone $this;
        $copy->sorts[] = $sort;
        return $copy;
    }

    private static function direction(string $direction): string
    {
        return match (strtolower($direction)) {
            'asc' => 'Asc',
            'desc' => 'Desc',
            default => throw new InvalidArgumentException("sort direction must be 'asc' or 'desc'"),
        };
    }

    /** @return array<string,mixed> */
    private static function missingPolicy(mixed $missing): array
    {
        if ($missing === 'first') {
            return ['policy_type' => 'MissingFirst', 'policy' => []];
        }
        if ($missing === 'last') {
            return ['policy_type' => 'MissingLast', 'policy' => []];
        }
        return [
            'policy_type' => 'MissingAs',
            'policy' => ['value' => Protocol::value($missing)],
        ];
    }

    private static function uint32(int $value, string $label): void
    {
        if ($value < 0 || $value > 0xFFFF_FFFF) {
            throw new InvalidArgumentException($label . ' must be a uint32');
        }
    }
}
