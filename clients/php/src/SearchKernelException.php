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
        $code = $payload['code'] ?? null;
        $message = $payload['message'] ?? null;
        $retryable = $payload['retryable'] ?? null;
        $details = $payload['details'] ?? [];

        if (!is_string($code) || $code === '') {
            throw new ProtocolException('canonical Error.code must be a non-empty enum name');
        }
        if (!is_string($message)) {
            throw new ProtocolException('canonical Error.message must be a string');
        }
        if (!is_bool($retryable)) {
            throw new ProtocolException('canonical Error.retryable must be a boolean');
        }
        if (!is_array($details) || !array_is_list($details)) {
            throw new ProtocolException('canonical Error.details must be a list');
        }

        foreach ($details as $detail) {
            if (!is_array($detail)) {
                throw new ProtocolException('canonical Error detail must be an object');
            }
            if (!is_string($detail['code'] ?? null) || !is_string($detail['message'] ?? null)) {
                throw new ProtocolException('canonical Error detail code/message must be strings');
            }
            if (array_key_exists('path', $detail) && !is_string($detail['path'])) {
                throw new ProtocolException('canonical Error detail path must be a string');
            }
            if (array_key_exists('metadata', $detail) && !is_array($detail['metadata'])) {
                throw new ProtocolException('canonical Error detail metadata must be a list');
            }
        }

        return new self($code, $message, array_values($details), $retryable);
    }
}
