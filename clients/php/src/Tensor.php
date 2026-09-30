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
        try {
            return match ($payloadType) {
                'DenseTensorValues' => self::denseFromWire($typeSpec, $payload),
                'SparseTensorCells' => self::sparseFromWire($typeSpec, $payload),
                'MixedTensorBlocks' => self::mixedFromWire($typeSpec, $payload),
                default => throw new ProtocolException('unsupported tensor payload type: ' . $payloadType),
            };
        } catch (InvalidArgumentException|\TypeError $error) {
            throw new ProtocolException('malformed TensorValue payload', 0, $error);
        }
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

    /** @param array<string,mixed> $payload */
    private static function denseFromWire(string $typeSpec, array $payload): self
    {
        if ((array_keys($payload) !== [] && array_keys($payload) !== ['values'])
            || (array_key_exists('values', $payload)
                && (!is_array($payload['values']) || !array_is_list($payload['values'])))
        ) {
            throw new InvalidArgumentException('DenseTensorValues payload must contain only values');
        }
        return self::dense($typeSpec, $payload['values'] ?? []);
    }

    /** @param array<string,mixed> $payload */
    private static function sparseFromWire(string $typeSpec, array $payload): self
    {
        if (array_keys($payload) !== ['cells']
            || !is_array($payload['cells'])
            || !array_is_list($payload['cells'])
        ) {
            throw new InvalidArgumentException('SparseTensorCells payload must contain only cells');
        }
        $cells = [];
        foreach ($payload['cells'] as $cell) {
            if (!is_array($cell) || !is_array($cell['address'] ?? null)) {
                throw new InvalidArgumentException('malformed sparse tensor cell');
            }
            $cells[] = [
                'address' => self::wireAddress($cell['address']),
                'value' => Protocol::finiteScalarField(
                    $cell,
                    'value',
                    'TensorCell.value',
                ),
            ];
        }
        return self::sparse($typeSpec, $cells);
    }

    /** @param array<string,mixed> $payload */
    private static function mixedFromWire(string $typeSpec, array $payload): self
    {
        if (array_keys($payload) !== ['blocks']
            || !is_array($payload['blocks'])
            || !array_is_list($payload['blocks'])
        ) {
            throw new InvalidArgumentException('MixedTensorBlocks payload must contain only blocks');
        }
        $blocks = [];
        foreach ($payload['blocks'] as $block) {
            if (!is_array($block)
                || !is_array($block['address'] ?? null)
                || !array_is_list($block['address'] ?? [])
                || !is_array($block['values'] ?? null)
                || !array_is_list($block['values'] ?? [])
            ) {
                throw new InvalidArgumentException('malformed mixed tensor block');
            }
            $blocks[] = [
                'address' => self::wireAddress($block['address']),
                'values' => array_map(self::wireNumber(...), $block['values']),
            ];
        }
        return self::mixed($typeSpec, $blocks);
    }

    /** @param list<array{dimension:mixed,label:mixed}> $address
     *  @return array<string,string>
     */
    private static function wireAddress(array $address): array
    {
        if (!array_is_list($address)) {
            throw new InvalidArgumentException('tensor address must be a list');
        }
        $decoded = [];
        foreach ($address as $entry) {
            if (!is_array($entry)
                || !is_string($entry['dimension'] ?? null)
                || !is_string($entry['label'] ?? null)
                || array_key_exists($entry['dimension'], $decoded)
            ) {
                throw new InvalidArgumentException('malformed or duplicate tensor address entry');
            }
            $decoded[$entry['dimension']] = $entry['label'];
        }
        return $decoded;
    }

    private static function wireNumber(mixed $value): float
    {
        if ((!is_int($value) && !is_float($value)) || !is_finite((float) $value)) {
            throw new InvalidArgumentException('tensor cells must be finite numbers');
        }
        return (float) $value;
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
