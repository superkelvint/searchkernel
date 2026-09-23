<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Tensor
{
    /** @param array<string, mixed> $payload */
    private function __construct(
        public readonly string $typeSpec,
        public readonly string $payloadType,
        public readonly array $payload,
    ) {
        if (trim($typeSpec) === '') {
            throw new InvalidArgumentException('tensor type spec must be non-empty');
        }
    }

    /** @param array<string,mixed> $payload */
    public static function fromWire(string $typeSpec, string $payloadType, array $payload): self
    {
        if (!in_array($payloadType, ['DenseTensorValues', 'SparseTensorCells', 'MixedTensorBlocks'], true)) {
            throw new InvalidArgumentException('unsupported tensor payload type: ' . $payloadType);
        }
        return new self($typeSpec, $payloadType, $payload);
    }

    /** @param list<int|float> $values */
    public static function dense(string $typeSpec, array $values): self
    {
        return new self($typeSpec, 'DenseTensorValues', [
            'values' => self::finiteValues($values),
        ]);
    }

    /**
     * @param list<array{address:array<string,string>,value:int|float}> $cells
     */
    public static function sparse(string $typeSpec, array $cells): self
    {
        $encoded = [];
        foreach ($cells as $cell) {
            if (!isset($cell['address']) || !is_array($cell['address']) || !array_key_exists('value', $cell)) {
                throw new InvalidArgumentException('sparse tensor cells require address and value');
            }
            $encoded[] = [
                'address' => self::address($cell['address']),
                'value' => self::finite((float) $cell['value']),
            ];
        }
        return new self($typeSpec, 'SparseTensorCells', ['cells' => $encoded]);
    }

    /**
     * @param list<array{address:array<string,string>,values:list<int|float>}> $blocks
     */
    public static function mixed(string $typeSpec, array $blocks): self
    {
        $encoded = [];
        foreach ($blocks as $block) {
            if (!isset($block['address']) || !is_array($block['address']) || !isset($block['values']) || !is_array($block['values'])) {
                throw new InvalidArgumentException('mixed tensor blocks require address and values');
            }
            $encoded[] = [
                'address' => self::address($block['address']),
                'values' => self::finiteValues($block['values']),
            ];
        }
        return new self($typeSpec, 'MixedTensorBlocks', ['blocks' => $encoded]);
    }

    /** @param array<string,string> $address
     *  @return list<array{dimension:string,label:string}>
     */
    private static function address(array $address): array
    {
        ksort($address, SORT_STRING);
        $encoded = [];
        foreach ($address as $dimension => $label) {
            if (!is_string($dimension) || !is_string($label)) {
                throw new InvalidArgumentException('tensor addresses must map string dimensions to string labels');
            }
            $encoded[] = ['dimension' => $dimension, 'label' => $label];
        }
        return $encoded;
    }

    /** @param list<int|float> $values
     *  @return list<float>
     */
    private static function finiteValues(array $values): array
    {
        return array_map(static fn (int|float $value): float => self::finite((float) $value), $values);
    }

    private static function finite(float $value): float
    {
        if (!is_finite($value)) {
            throw new InvalidArgumentException('tensor cells must be finite');
        }
        return $value;
    }
}
