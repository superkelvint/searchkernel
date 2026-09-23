<?php

declare(strict_types=1);

namespace SearchKernel;

interface Transport
{
    /**
     * @param array<string, mixed> $request standard FlatBuffers JSON envelope
     * @return array<string, mixed> standard FlatBuffers JSON envelope
     */
    public function execute(array $request): array;
}
