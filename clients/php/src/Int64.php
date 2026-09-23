<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Int64
{
    private const MIN = '9223372036854775808';
    private const MAX = '9223372036854775807';

    public readonly string $value;

    public function __construct(string $value)
    {
        if (!preg_match('/^(?:0|-?[1-9][0-9]*)$/D', $value)) {
            throw new InvalidArgumentException('int64 must be a canonical decimal string');
        }

        $negative = str_starts_with($value, '-');
        $digits = $negative ? substr($value, 1) : $value;
        $limit = $negative ? self::MIN : self::MAX;
        if (strlen($digits) > strlen($limit)
            || (strlen($digits) === strlen($limit) && strcmp($digits, $limit) > 0)) {
            throw new InvalidArgumentException('int64 is outside the signed 64-bit range');
        }

        $this->value = $value;
    }

    public static function of(int|string $value): self
    {
        return new self(is_int($value) ? (string) $value : $value);
    }

    public function __toString(): string
    {
        return $this->value;
    }
}
