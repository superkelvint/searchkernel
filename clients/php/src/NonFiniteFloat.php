<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class NonFiniteFloat
{
    public const NAN = 'NaN';
    public const POSITIVE_INFINITY = 'PositiveInfinity';
    public const NEGATIVE_INFINITY = 'NegativeInfinity';

    public readonly string $kind;

    public function __construct(string $kind)
    {
        if (!in_array($kind, [self::NAN, self::POSITIVE_INFINITY, self::NEGATIVE_INFINITY], true)) {
            throw new InvalidArgumentException('unknown non-finite float kind');
        }
        $this->kind = $kind;
    }

    public static function nan(): self { return new self(self::NAN); }
    public static function positiveInfinity(): self { return new self(self::POSITIVE_INFINITY); }
    public static function negativeInfinity(): self { return new self(self::NEGATIVE_INFINITY); }
}
