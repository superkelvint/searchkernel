<?php

declare(strict_types=1);

namespace SearchKernel;

final class ResultIssue
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $code = $node['code'] ?? null;
        $summary = $node['summary'] ?? null;
        $message = $node['message'] ?? null;
        $source = $node['source'] ?? null;
        $stackTrace = $node['stack_trace'] ?? null;

        if ((!is_int($code) && !is_string($code))
            || !is_string($summary)
            || !is_string($message)
            || !is_string($source)
            || !is_string($stackTrace)
        ) {
            throw new ProtocolException('malformed SearchResult error');
        }

        $this->code = $code;
        $this->summary = $summary;
        $this->message = $message;
        $this->source = $source;
        $this->stackTrace = $stackTrace;

        if (array_key_exists('transient', $node) && $node['transient'] !== null) {
            if (!is_array($node['transient'])
                || !array_key_exists('value', $node['transient'])
                || !is_bool($node['transient']['value'])
            ) {
                throw new ProtocolException('result error transient must be an optional boolean');
            }
            $this->transient = $node['transient']['value'];
        } else {
            $this->transient = null;
        }
        $this->metadata = Protocol::decodeValueEntries($node['metadata'] ?? []);
    }

    public readonly int|string $code;
    public readonly string $summary;
    public readonly string $message;
    public readonly string $source;
    public readonly string $stackTrace;
    public readonly ?bool $transient;
    /** @var array<string,mixed> */
    public readonly array $metadata;
}
