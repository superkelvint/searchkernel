<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class UInt64
{
    private const MAX = '18446744073709551615';

    public readonly string $value;

    public function __construct(string $value)
    {
        if (!preg_match('/^(?:0|[1-9][0-9]*)$/D', $value)) {
            throw new InvalidArgumentException('uint64 must be a canonical unsigned decimal string');
        }
        if (strlen($value) > strlen(self::MAX)
            || (strlen($value) === strlen(self::MAX) && strcmp($value, self::MAX) > 0)) {
            throw new InvalidArgumentException('uint64 is outside the unsigned 64-bit range');
        }

        $this->value = $value;
    }

    public static function of(int|string $value): self
    {
        if (is_int($value) && $value < 0) {
            throw new InvalidArgumentException('uint64 must not be negative');
        }
        return new self(is_int($value) ? (string) $value : $value);
    }

    public function __toString(): string
    {
        return $this->value;
    }
}
