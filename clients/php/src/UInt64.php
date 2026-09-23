<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class UInt64
{
    public readonly string $value;

    public function __construct(string|int $value)
    {
        $decimal = (string) $value;
        if (!preg_match('/^(0|[1-9][0-9]*)$/', $decimal)) {
            throw new InvalidArgumentException('uint64 must be an unsigned decimal integer');
        }
        $decimal = ltrim($decimal, '0');
        $decimal = $decimal === '' ? '0' : $decimal;
        $max = '18446744073709551615';
        if (strlen($decimal) > strlen($max)
            || (strlen($decimal) === strlen($max) && strcmp($decimal, $max) > 0)
        ) {
            throw new InvalidArgumentException('uint64 exceeds 18446744073709551615');
        }
        $this->value = $decimal;
    }
}
