<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Bytes
{
    public function __construct(public readonly string $value) {}

    /** @return list<int> */
    public function octets(): array
    {
        $out = [];
        for ($i = 0, $length = strlen($this->value); $i < $length; $i++) {
            $out[] = ord($this->value[$i]);
        }
        return $out;
    }

    /** @param list<int> $octets */
    public static function fromOctets(array $octets): self
    {
        $value = '';
        foreach ($octets as $octet) {
            if (!is_int($octet) || $octet < 0 || $octet > 255) {
                throw new InvalidArgumentException('byte values must be integers in 0..255');
            }
            $value .= chr($octet);
        }
        return new self($value);
    }
}
