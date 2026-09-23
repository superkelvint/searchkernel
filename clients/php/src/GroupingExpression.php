<?php

declare(strict_types=1);

namespace SearchKernel;

final class GroupingExpression
{
    /** @param array<string,mixed> $model */
    public function __construct(public readonly array $model) {}

    public function as(string $label): GroupingOutput
    {
        return new GroupingOutput($this, $label);
    }
}
