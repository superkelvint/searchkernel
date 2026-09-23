<?php

declare(strict_types=1);

namespace SearchKernel;

/**
 * Restricted YQL builder.
 *
 * Raw YQL owns filtering, sorting, search-field selection and SELECT/output
 * projection. This wrapper deliberately exposes only request-level controls
 * which may coexist with YQL, instead of inheriting the full SearchBuilder API.
 */
final class YqlBuilder
{
    private SearchBuilder $builder;

    /** @param array<string,mixed> $query */
    public function __construct(Index $index, array $query)
    {
        $this->builder = new SearchBuilder($index, $query);
    }

    public function property(string $name, mixed $value): self
    {
        return $this->with($this->builder->property($name, $value));
    }

    /** @param array<string,mixed> $properties */
    public function properties(array $properties): self
    {
        return $this->with($this->builder->properties($properties));
    }

    public function group(Group $group): self
    {
        return $this->with($this->builder->group($group));
    }

    public function rawGrouping(string $expression): self
    {
        return $this->with($this->builder->rawGrouping($expression));
    }

    public function ranking(string $profile): self
    {
        return $this->with($this->builder->ranking($profile));
    }

    public function rankProperty(string $name, mixed $value): self
    {
        return $this->with($this->builder->rankProperty($name, $value));
    }

    /** @param array<string,mixed> $properties */
    public function rankProperties(array $properties): self
    {
        return $this->with($this->builder->rankProperties($properties));
    }

    public function input(string $name, mixed $value): self
    {
        return $this->with($this->builder->input($name, $value));
    }

    public function inputExpr(string $name, string $expression): self
    {
        return $this->with($this->builder->inputExpr($name, $expression));
    }

    public function offset(int $offset): self
    {
        return $this->with($this->builder->offset($offset));
    }

    public function limit(int $limit): self
    {
        return $this->with($this->builder->limit($limit));
    }

    public function page(int $number, int $size): self
    {
        return $this->with($this->builder->page($number, $size));
    }

    public function timeout(int $milliseconds): self
    {
        return $this->with($this->builder->timeout($milliseconds));
    }

    public function execute(): SearchResult
    {
        return $this->builder->execute();
    }

    /** @return array<string,mixed> */
    public function inspect(): array
    {
        return $this->builder->inspect();
    }

    /** @return array<string,mixed> */
    public function request(): array
    {
        return $this->builder->request();
    }

    private function with(SearchBuilder $builder): self
    {
        $copy = clone $this;
        $copy->builder = $builder;
        return $copy;
    }
}
