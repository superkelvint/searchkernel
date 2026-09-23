<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Tensor
{
    public const DENSE = 'DenseTensorValues';
    public const SPARSE = 'SparseTensorCells';
    public const MIXED = 'MixedTensorBlocks';

    /** @param array<string, mixed> $payload */
    private function __construct(
        public readonly string $typeSpec,
        public readonly string $payloadType,
        public readonly array $payload,
    ) {}

    /** @param list<int|float> $values */
    public static function dense(string $typeSpec, array $values): self
    {
        return new self($typeSpec, self::DENSE, ['values' => self::finiteValues($values)]);
    }

    /**
     * @param list<array{address:array<string,string>,value:int|float}> $cells
     */
    public static function sparse(string $typeSpec, array $cells): self
    {
        $encoded = [];
        foreach ($cells as $cell) {
            if (!is_array($cell) || !array_key_exists('address', $cell) || !array_key_exists('value', $cell)) {
                throw new InvalidArgumentException('sparse tensor cells require address and value');
            }
            $encoded[] = [
                'address' => self::address($cell['address']),
                'value' => self::finiteNumber($cell['value']),
            ];
        }
        return new self($typeSpec, self::SPARSE, ['cells' => $encoded]);
    }

    /**
     * @param list<array{address:array<string,string>,values:list<int|float>}> $blocks
     */
    public static function mixed(string $typeSpec, array $blocks): self
    {
        $encoded = [];
        foreach ($blocks as $block) {
            if (!is_array($block) || !array_key_exists('address', $block) || !array_key_exists('values', $block)) {
                throw new InvalidArgumentException('mixed tensor blocks require address and values');
            }
            $encoded[] = [
                'address' => self::address($block['address']),
                'values' => self::finiteValues($block['values']),
            ];
        }
        return new self($typeSpec, self::MIXED, ['blocks' => $encoded]);
    }

    /** @param array<string, mixed> $json */
    public static function fromJson(array $json): self
    {
        $typeSpec = $json['type_spec'] ?? null;
        $payloadType = $json['payload_type'] ?? null;
        $payload = $json['payload'] ?? null;
        if (!is_string($typeSpec) || !is_string($payloadType) || !is_array($payload)) {
            throw new ProtocolException('malformed TensorValue');
        }

        try {
            return match ($payloadType) {
                self::DENSE => self::decodeDense($typeSpec, $payload),
                self::SPARSE => self::decodeSparse($typeSpec, $payload),
                self::MIXED => self::decodeMixed($typeSpec, $payload),
                default => throw new ProtocolException('unsupported TensorValue payload kind: ' . $payloadType),
            };
        } catch (InvalidArgumentException $error) {
            throw new ProtocolException($error->getMessage(), 0, $error);
        }
    }

    /** @return array<string, mixed> */
    public function toJson(): array
    {
        return [
            'type_spec' => $this->typeSpec,
            'payload_type' => $this->payloadType,
            'payload' => $this->payload,
        ];
    }

    /** @param array<string, mixed> $payload */
    private static function decodeDense(string $typeSpec, array $payload): self
    {
        $values = $payload['values'] ?? null;
        if (!is_array($values) || !array_is_list($values)) {
            throw new ProtocolException('DenseTensorValues.values must be a list');
        }
        return self::dense($typeSpec, $values);
    }

    /** @param array<string, mixed> $payload */
    private static function decodeSparse(string $typeSpec, array $payload): self
    {
        $cells = $payload['cells'] ?? null;
        if (!is_array($cells) || !array_is_list($cells)) {
            throw new ProtocolException('SparseTensorCells.cells must be a list');
        }
        $decoded = [];
        foreach ($cells as $cell) {
            if (!is_array($cell)) {
                throw new ProtocolException('sparse tensor cell must be an object');
            }
            $decoded[] = [
                'address' => self::decodeAddress($cell['address'] ?? null),
                'value' => self::protocolFiniteNumber($cell['value'] ?? null),
            ];
        }
        return self::sparse($typeSpec, $decoded);
    }

    /** @param array<string, mixed> $payload */
    private static function decodeMixed(string $typeSpec, array $payload): self
    {
        $blocks = $payload['blocks'] ?? null;
        if (!is_array($blocks) || !array_is_list($blocks)) {
            throw new ProtocolException('MixedTensorBlocks.blocks must be a list');
        }
        $decoded = [];
        foreach ($blocks as $block) {
            if (!is_array($block)) {
                throw new ProtocolException('mixed tensor block must be an object');
            }
            $values = $block['values'] ?? null;
            if (!is_array($values) || !array_is_list($values)) {
                throw new ProtocolException('mixed tensor block values must be a list');
            }
            $decoded[] = [
                'address' => self::decodeAddress($block['address'] ?? null),
                'values' => array_map(self::protocolFiniteNumber(...), $values),
            ];
        }
        return self::mixed($typeSpec, $decoded);
    }

    /** @param array<string,string> $address
     *  @return list<array{dimension:string,label:string}>
     */
    private static function address(array $address): array
    {
        ksort($address, SORT_STRING);
        $result = [];
        foreach ($address as $dimension => $label) {
            if (!is_string($dimension) || !is_string($label)) {
                throw new InvalidArgumentException('tensor address dimensions and labels must be strings');
            }
            $result[] = ['dimension' => $dimension, 'label' => $label];
        }
        return $result;
    }

    /** @return array<string,string> */
    private static function decodeAddress(mixed $entries): array
    {
        if (!is_array($entries) || !array_is_list($entries)) {
            throw new ProtocolException('tensor address must be a list');
        }
        $result = [];
        foreach ($entries as $entry) {
            if (!is_array($entry)
                || !is_string($entry['dimension'] ?? null)
                || !is_string($entry['label'] ?? null)) {
                throw new ProtocolException('malformed tensor address entry');
            }
            $dimension = $entry['dimension'];
            if (array_key_exists($dimension, $result)) {
                throw new ProtocolException('duplicate tensor address dimension: ' . $dimension);
            }
            $result[$dimension] = $entry['label'];
        }
        return $result;
    }

    /** @param list<int|float> $values
     *  @return list<float>
     */
    private static function finiteValues(array $values): array
    {
        if (!array_is_list($values)) {
            throw new InvalidArgumentException('tensor values must be a list');
        }
        return array_map(self::finiteNumber(...), $values);
    }

    private static function finiteNumber(mixed $value): float
    {
        if ((!is_int($value) && !is_float($value)) || !is_finite((float) $value)) {
            throw new InvalidArgumentException('tensor cell values must be finite numbers');
        }
        return (float) $value;
    }

    private static function protocolFiniteNumber(mixed $value): float
    {
        try {
            return self::finiteNumber($value);
        } catch (InvalidArgumentException $error) {
            throw new ProtocolException($error->getMessage(), 0, $error);
        }
    }
}
