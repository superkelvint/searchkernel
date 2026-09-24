<?php

declare(strict_types=1);

namespace SearchKernel;

final class GroupingOutput
{
    public function __construct(
        public readonly GroupingExpression $expression,
        public readonly string $label = '',
    ) {}

    /** @return array<string,mixed> */
    public function toWire(): array
    {
        $wire = ['expression' => $this->expression->model];
        if ($this->label !== '') {
            $wire['label'] = $this->label;
        }
        return $wire;
    }
}
