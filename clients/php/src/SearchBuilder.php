<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class SearchBuilder
{
    private ?int $limit = null;

    public function __construct(
        private readonly Index $index,
        private readonly string $text,
    ) {
        if (trim($text) === '') {
            throw new InvalidArgumentException('search text must not be empty');
        }
    }

    public function limit(int $limit): self
    {
        if ($limit < 0 || $limit > 0xFFFF_FFFF) {
            throw new InvalidArgumentException('limit must be a uint32');
        }
        $copy = clone $this;
        $copy->limit = $limit;
        return $copy;
    }

    public function execute(): SearchResult
    {
        return $this->index->executeSearch($this->text, $this->limit);
    }
}
