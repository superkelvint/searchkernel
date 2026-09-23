<?php

declare(strict_types=1);

namespace SearchKernel;

use UnexpectedValueException;

final class ResultIssue
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->code = $node['code'] ?? 0;
        $this->summary = (string) ($node['summary'] ?? '');
        $this->message = (string) ($node['message'] ?? '');
        $this->source = (string) ($node['source'] ?? '');
        $this->stackTrace = (string) ($node['stack_trace'] ?? '');
        if (array_key_exists('transient', $node) && $node['transient'] !== null) {
            if (!is_array($node['transient']) || !array_key_exists('value', $node['transient'])) {
                throw new UnexpectedValueException('result error transient must be an optional-value object');
            }
            $this->transient = (bool) $node['transient']['value'];
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
