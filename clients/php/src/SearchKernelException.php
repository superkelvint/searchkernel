<?php

declare(strict_types=1);

namespace SearchKernel;

use RuntimeException;

final class SearchKernelException extends RuntimeException
{
    /** @param list<array{code:string,path:?string,message:string,metadata:array<string,string>}> $details */
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

        if (!is_string($code) || !is_string($message) || !is_bool($retryable) || !is_array($details)) {
            throw new ProtocolException('malformed canonical Search Kernel Error payload');
        }

        $decodedDetails = [];
        foreach ($details as $detail) {
            if (!is_array($detail)
                || !is_string($detail['code'] ?? null)
                || !is_string($detail['message'] ?? null)
                || (array_key_exists('path', $detail) && $detail['path'] !== null && !is_string($detail['path']))
                || !is_array($detail['metadata'] ?? [])
            ) {
                throw new ProtocolException('malformed canonical Search Kernel Error detail');
            }

            $metadata = [];
            foreach ($detail['metadata'] ?? [] as $entry) {
                if (!is_array($entry)
                    || !is_string($entry['key'] ?? null)
                    || !is_string($entry['value'] ?? null)
                    || array_key_exists($entry['key'], $metadata)
                ) {
                    throw new ProtocolException('malformed or duplicate Search Kernel Error detail metadata');
                }
                $metadata[$entry['key']] = $entry['value'];
            }

            $decodedDetails[] = [
                'code' => $detail['code'],
                'path' => $detail['path'] ?? null,
                'message' => $detail['message'],
                'metadata' => $metadata,
            ];
        }

        return new self($code, $message, $decodedDetails, $retryable);
    }
}
