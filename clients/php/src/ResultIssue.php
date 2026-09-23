<?php

declare(strict_types=1);

namespace SearchKernel;

final class ResultIssue
{
    /** @param array<string,mixed> $node */
    public function __construct(array $node)
    {
        $this->code = self::integerLike($node, 'code', 0);
        $this->summary = self::string($node, 'summary', '');
        $this->message = self::string($node, 'message', '');
        $this->source = self::string($node, 'source', '');
        $this->stackTrace = self::string($node, 'stack_trace', '');

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

    /** @param array<string,mixed> $node */
    private static function string(array $node, string $key, string $default): string
    {
        if (!array_key_exists($key, $node)) {
            return $default;
        }
        if (!is_string($node[$key])) {
            throw new ProtocolException('SearchResultError.' . $key . ' must be a string');
        }
        return $node[$key];
    }

    /** @param array<string,mixed> $node */
    private static function integerLike(array $node, string $key, int $default): int|string
    {
        if (!array_key_exists($key, $node)) {
            return $default;
        }
        return Protocol::decodeInt64Scalar($node[$key], 'SearchResultError.' . $key);
    }
}
