<?php

declare(strict_types=1);

namespace SearchKernel;

interface Transport
{
    /** @param array<string, mixed> $request
     *  @return array<string, mixed>
     */
    public function execute(array $request): array;
}
