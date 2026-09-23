<?php

declare(strict_types=1);

namespace SearchKernel;

use RuntimeException;

final class SearchKernelException extends RuntimeException
{
    /** @param list<array<string, mixed>> $details */
    public function __construct(
        public readonly string $errorCode,
        string $message,
        public readonly array $details = [],
        public readonly bool $retryable = false,
    ) {
        parent::__construct($message);
    }

    /** @param array<string, mixed> $payload */
    public static function fromPayload(array $payload): self
    {
        return new self(
            (string) ($payload['code'] ?? 'Unknown'),
            (string) ($payload['message'] ?? 'Search Kernel request failed'),
            is_array($payload['details'] ?? null) ? array_values($payload['details']) : [],
            (bool) ($payload['retryable'] ?? false),
        );
    }
}
