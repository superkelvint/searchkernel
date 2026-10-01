<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Protocol
{
    public const VERSION = 1;

    /** @var array<string,array{0:string,1:string}> */
    private const OPERATIONS = [
        'insert' => ['InsertRequest', 'InsertResult'],
        'upsert' => ['UpsertRequest', 'UpsertResult'],
        'patch' => ['PatchRequest', 'PatchResult'],
        'update' => ['UpdateRequest', 'UpdateResult'],
        'delete' => ['DeleteRequest', 'DeleteResult'],
        'get' => ['GetRequest', 'GetResult'],
        'search' => ['SearchRequest', 'SearchResult'],
        'analyze' => ['AnalyzeRequest', 'AnalyzeResult'],
        'describe' => ['DescribeRequest', 'DescribeResult'],
        'inspect_query' => ['InspectQueryRequest', 'InspectQueryResult'],
        'create_index' => ['CreateIndexRequest', 'CreateIndexResult'],
        'open_index' => ['OpenIndexRequest', 'OpenIndexResult'],
        'delete_index' => ['DeleteIndexRequest', 'DeleteIndexResult'],
    ];

    /** @return array{0:string,1:string} */
    public static function operation(string $operation): array
    {
        if (!array_key_exists($operation, self::OPERATIONS)) {
            throw new InvalidArgumentException('unknown Search Kernel operation: ' . $operation);
        }
        return self::OPERATIONS[$operation];
    }

    /** @return list<string> */
    public static function operations(): array
    {
        return array_keys(self::OPERATIONS);
    }

    /** @return array<string, mixed> */
    public static function envelope(
        string $payloadType,
        array|JsonObject $payload,
        ?string $target = null,
    ): array
    {
        $envelope = [
            'protocol_version' => self::VERSION,
            'payload_type' => 'SearchKernel_Wire_' . $payloadType,
            'payload' => $payload,
        ];
        if ($target !== null) {
            $envelope['target'] = ['name' => $target];
        }
        return $envelope;
    }

    /** @param array<string, mixed> $document
     *  @return array<string, mixed>
     */
    public static function document(array $document): array
    {
        $id = $document['_id'] ?? null;
        if (!is_string($id) || $id === '') {
            throw new InvalidArgumentException('document _id must be a non-empty string');
        }

        unset($document['_id']);

        return [
            'id' => $id,
            'fields' => self::valueEntries($document),
        ];
    }

    /** @param array<string, mixed> $values
     *  @return list<array{key:string,value:array<string,mixed>}>
     */
    public static function valueEntries(array $values): array
    {
        ksort($values, SORT_STRING);
        $entries = [];
        foreach ($values as $key => $value) {
            if (!is_string($key)) {
                throw new InvalidArgumentException('portable map keys must be strings');
            }
            $entries[] = ['key' => $key, 'value' => self::value($value)];
        }
        return $entries;
    }

    /** @param array<string,string> $values
     *  @return list<array{key:string,value:string}>
     */
    public static function stringEntries(array $values): array
    {
        ksort($values, SORT_STRING);
        $entries = [];
        foreach ($values as $key => $value) {
            if (!is_string($key) || !is_string($value)) {
                throw new InvalidArgumentException('string entry maps require string keys and values');
            }
            $entries[] = ['key' => $key, 'value' => $value];
        }
        return $entries;
    }

    /** @return array<string, mixed> */
    public static function value(mixed $value): array
    {
        if ($value === null) {
            return [
                'kind_type' => 'NullValueValue',
                'kind' => ['value' => 'Null'],
            ];
        }
        if (is_bool($value)) {
            return [
                'kind_type' => 'BoolValue',
                'kind' => ['value' => $value],
            ];
        }
        if ($value instanceof UInt64) {
            return [
                'kind_type' => 'UInt64Value',
                'kind' => ['value' => $value->value],
            ];
        }
        if ($value instanceof ValueObject) {
            return [
                'kind_type' => 'ValueObject',
                'kind' => [
                    'fields' => array_map(
                        static fn (array $entry): array => [
                            'key' => $entry['key'],
                            'value' => self::value($entry['value']),
                        ],
                        $value->entries,
                    ),
                ],
            ];
        }
        if (is_int($value)) {
            return [
                'kind_type' => 'Int64Value',
                'kind' => ['value' => (string) $value],
            ];
        }
        if (is_float($value)) {
            if (!is_finite($value)) {
                throw new InvalidArgumentException('portable float values must be finite');
            }
            return [
                'kind_type' => 'Float64Value',
                'kind' => ['value' => $value],
            ];
        }
        if (is_string($value)) {
            return [
                'kind_type' => 'StringValue',
                'kind' => ['value' => $value],
            ];
        }
        if ($value instanceof Bytes) {
            $bytes = unpack('C*', $value->value);
            return [
                'kind_type' => 'BytesValue',
                'kind' => ['value' => $bytes === false ? [] : array_values($bytes)],
            ];
        }
        if ($value instanceof Tensor) {
            $payload = $value->payload;
            if ($value->payloadType === 'DenseTensorValues' && ($payload['values'] ?? null) === []) {
                // FlatBuffers JSON parsers can produce a misaligned empty
                // vector of doubles. An absent vector decodes as the same
                // empty dense payload in the canonical IR.
                $payload = JsonObject::empty();
            }
            return [
                'kind_type' => 'TensorValue',
                'kind' => [
                    'type_spec' => $value->typeSpec,
                    'payload_type' => $value->payloadType,
                    'payload' => $payload,
                ],
            ];
        }
        if (is_array($value)) {
            if (array_is_list($value)) {
                return [
                    'kind_type' => 'ValueList',
                    'kind' => ['values' => array_map(self::value(...), $value)],
                ];
            }
            return [
                'kind_type' => 'ValueObject',
                'kind' => ['fields' => self::valueEntries($value)],
            ];
        }

        throw new InvalidArgumentException(
            'unsupported PHP value; first HTTP/JSON slice supports null, bool, int, float, string, list, and associative array'
        );
    }

    /** @param array<string, mixed> $response
     *  @return array<string, mixed>
     */
    public static function expectPayload(array $response, string $expectedType): array
    {
        if (($response['protocol_version'] ?? null) !== self::VERSION) {
            throw new ProtocolException('unsupported Search Kernel response protocol version');
        }

        $type = $response['payload_type'] ?? null;
        $payload = $response['payload'] ?? null;
        if (!is_array($payload)) {
            throw new ProtocolException('Search Kernel response is missing its union payload');
        }

        if ($type === 'SearchKernel_Wire_Error') {
            throw SearchKernelException::fromPayload($payload);
        }

        if ($type !== 'SearchKernel_Wire_' . $expectedType) {
            throw new ProtocolException(sprintf(
                'unexpected Search Kernel response type %s (expected %s)',
                is_string($type) ? $type : '<missing>',
                'SearchKernel_Wire_' . $expectedType,
            ));
        }

        return $payload;
    }

    /** @param array<string, mixed> $document
     *  @return array<string, mixed>
     */
    public static function decodeDocument(array $document): array
    {
        $fields = self::decodeValueEntries(
            self::listField($document, 'fields', 'Document.fields'),
        );
        $id = $document['id'] ?? null;
        if (!is_string($id)) {
            throw new ProtocolException('Document.id must be a string');
        }
        $fields['_id'] = $id;
        return $fields;
    }

    /** @param mixed $entries
     *  @return array<string, mixed>
     */
    public static function decodeValueEntries(mixed $entries): array
    {
        if (!is_array($entries) || !array_is_list($entries)) {
            throw new ProtocolException('portable value entries must be a list');
        }

        $result = [];
        foreach ($entries as $entry) {
            if (!is_array($entry) || !is_string($entry['key'] ?? null) || !is_array($entry['value'] ?? null)) {
                throw new ProtocolException('malformed portable value entry');
            }
            $key = $entry['key'];
            if (array_key_exists($key, $result)) {
                throw new ProtocolException('duplicate portable value key: ' . $key);
            }
            $result[$key] = self::decodeValue($entry['value']);
        }
        return $result;
    }

    /** @param array<string, mixed> $value */
    public static function decodeValue(array $value): mixed
    {
        $type = $value['kind_type'] ?? null;
        $payload = $value['kind'] ?? null;
        if (!is_string($type) || !is_array($payload)) {
            throw new ProtocolException('malformed portable Value union');
        }

        return match ($type) {
            'NullValueValue' => self::decodeNull($payload),
            'BoolValue' => self::decodeBool($payload),
            'Int64Value' => self::decodeInteger($payload, true),
            'UInt64Value' => self::decodeInteger($payload, false),
            'Float64Value' => self::decodeFloat64($payload),
            'NonFiniteFloatValue' => self::decodeNonFiniteFloat($payload),
            'StringValue' => self::decodeString($payload),
            'BytesValue' => self::decodeBytes($payload),
            'ValueList' => self::decodeValueList($payload),
            'ValueObject' => self::decodeValueObject($payload),
            'TensorValue' => self::decodeTensor($payload),
            default => throw new ProtocolException(
                'unsupported portable Value kind: ' . $type
            ),
        };
    }

    /** @param array<string, mixed> $node */
    public static function decodeRelevance(array $node): ?float
    {
        $type = $node['value_type'] ?? null;
        $payload = $node['value'] ?? null;
        if (!is_string($type) || !is_array($payload)) {
            throw new ProtocolException('malformed relevance union');
        }

        return match ($type) {
            'FiniteRelevance' => self::finiteScalarField($payload, 'value', 'FiniteRelevance.value', 0.0),
            'PositiveInfinityRelevance' => self::emptyMarker($payload, 'PositiveInfinityRelevance'),
            'NegativeInfinityRelevance' => -self::emptyMarker($payload, 'NegativeInfinityRelevance'),
            default => throw new ProtocolException('unsupported relevance kind: ' . $type),
        };
    }

    /** @param array<string, mixed> $payload
     *  @return list<mixed>
     */
    private static function decodeValueList(array $payload): array
    {
        $values = self::listField($payload, 'values', 'ValueList.values');
        return array_map(
            static function (mixed $value): mixed {
                if (!is_array($value)) {
                    throw new ProtocolException('malformed portable ValueList member');
                }
                return self::decodeValue($value);
            },
            $values,
        );
    }

    /** @param array<string,mixed> $payload */
    private static function decodeValueObject(array $payload): ValueObject
    {
        $entries = self::listField($payload, 'fields', 'ValueObject.fields');
        $pairs = [];
        $seen = [];
        foreach ($entries as $entry) {
            if (!is_array($entry) || !is_string($entry['key'] ?? null) || !is_array($entry['value'] ?? null)) {
                throw new ProtocolException('malformed ValueObject entry');
            }
            $key = $entry['key'];
            if (array_key_exists($key, $seen)) {
                throw new ProtocolException('duplicate portable value key: ' . $key);
            }
            $seen[$key] = true;
            $pairs[] = ['key' => $key, 'value' => self::decodeValue($entry['value'])];
        }
        return ValueObject::fromPairs($pairs);
    }

    /** @param array<string, mixed> $payload */
    private static function decodeNonFiniteFloat(array $payload): float
    {
        return match (self::required($payload, 'kind')) {
            'NaN' => NAN,
            'PositiveInfinity' => INF,
            'NegativeInfinity' => -INF,
            default => throw new ProtocolException('unknown non-finite float kind'),
        };
    }

    /** @param array<string, mixed> $payload */
    private static function decodeBytes(array $payload): Bytes
    {
        $values = self::required($payload, 'value');
        if (!is_array($values) || !array_is_list($values)) {
            throw new ProtocolException('BytesValue.value must be a list');
        }
        $binary = '';
        foreach ($values as $value) {
            if (!is_int($value) || $value < 0 || $value > 255) {
                throw new ProtocolException('BytesValue members must be uint8 values');
            }
            $binary .= chr($value);
        }
        return new Bytes($binary);
    }

    /** @param array<string, mixed> $payload */
    private static function decodeTensor(array $payload): Tensor
    {
        $typeSpec = self::required($payload, 'type_spec');
        $payloadType = self::required($payload, 'payload_type');
        $tensorPayload = self::required($payload, 'payload');
        if (!is_string($typeSpec) || !is_string($payloadType) || !is_array($tensorPayload)) {
            throw new ProtocolException('malformed TensorValue payload');
        }

        return Tensor::fromWire($typeSpec, $payloadType, $tensorPayload);
    }

    /** @param array<string,mixed> $payload */
    private static function decodeNull(array $payload): mixed
    {
        $value = self::required($payload, 'value');
        if ($value !== 'Null') {
            throw new ProtocolException('NullValueValue.value must be Null');
        }
        return null;
    }

    /** @param array<string,mixed> $payload */
    private static function decodeBool(array $payload): bool
    {
        return self::boolScalarField($payload, 'value', 'BoolValue.value', false);
    }

    /** @param array<string,mixed> $payload */
    private static function decodeString(array $payload): string
    {
        $value = self::required($payload, 'value');
        if (!is_string($value)) {
            throw new ProtocolException('StringValue.value must be a string');
        }
        return $value;
    }

    /** @param array<string,mixed> $payload */
    private static function decodeFloat64(array $payload): float
    {
        return self::finiteScalarField($payload, 'value', 'Float64Value.value', 0.0);
    }

    private static function finiteNumber(mixed $value, string $label): float
    {
        if ((!is_int($value) && !is_float($value)) || !is_finite((float) $value)) {
            throw new ProtocolException($label . ' must be a finite JSON number');
        }
        return (float) $value;
    }

    /** @param array<string,mixed> $payload */
    private static function decodeInteger(array $payload, bool $signed): int|string
    {
        $value = array_key_exists('value', $payload) ? $payload['value'] : 0;
        if (is_int($value)) {
            if (!$signed && $value < 0) {
                throw new ProtocolException('UInt64Value.value must be unsigned');
            }
            return $value;
        }
        if (!is_string($value)) {
            throw new ProtocolException(($signed ? 'Int64Value' : 'UInt64Value') . '.value must be an integer or decimal string');
        }
        $pattern = $signed ? '/^-?(0|[1-9][0-9]*)$/' : '/^(0|[1-9][0-9]*)$/';
        if (preg_match($pattern, $value) !== 1) {
            throw new ProtocolException('invalid integer decimal string');
        }

        if ($signed) {
            $negative = str_starts_with($value, '-');
            $digits = ltrim($negative ? substr($value, 1) : $value, '0');
            $digits = $digits === '' ? '0' : $digits;
            $max = $negative ? '9223372036854775808' : '9223372036854775807';
            if (strlen($digits) > strlen($max) || (strlen($digits) === strlen($max) && strcmp($digits, $max) > 0)) {
                throw new ProtocolException('Int64Value.value exceeds int64 range');
            }
            return (int) $value;
        }

        $digits = ltrim($value, '0');
        $digits = $digits === '' ? '0' : $digits;
        $max = '18446744073709551615';
        if (strlen($digits) > strlen($max) || (strlen($digits) === strlen($max) && strcmp($digits, $max) > 0)) {
            throw new ProtocolException('UInt64Value.value exceeds uint64 range');
        }
        if (strlen($digits) < strlen((string) PHP_INT_MAX)
            || (strlen($digits) === strlen((string) PHP_INT_MAX) && strcmp($digits, (string) PHP_INT_MAX) <= 0)
        ) {
            return (int) $digits;
        }
        return $digits;
    }

    public static function decodeInt64Scalar(mixed $value, string $label): int|string
    {
        try {
            return self::decodeInteger(['value' => $value], true);
        } catch (ProtocolException $error) {
            throw new ProtocolException($label . ': ' . $error->getMessage(), 0, $error);
        }
    }

    public static function decodeUInt64Scalar(mixed $value, string $label): int|string
    {
        try {
            return self::decodeInteger(['value' => $value], false);
        } catch (ProtocolException $error) {
            throw new ProtocolException($label . ': ' . $error->getMessage(), 0, $error);
        }
    }

    public static function decodeStringScalar(mixed $value, string $label): string
    {
        if (!is_string($value)) {
            throw new ProtocolException($label . ' must be a string');
        }
        return $value;
    }

    /** @param array<string,mixed> $node */
    public static function boolScalarField(array $node, string $key, string $label, bool $default = false): bool
    {
        if (!array_key_exists($key, $node)) {
            return $default;
        }
        if (!is_bool($node[$key])) {
            throw new ProtocolException($label . ' must be a boolean');
        }
        return $node[$key];
    }

    /** @param array<string,mixed> $node */
    public static function uint32ScalarField(array $node, string $key, string $label, int $default = 0): int
    {
        if (!array_key_exists($key, $node)) {
            return $default;
        }
        $value = $node[$key];
        if (!is_int($value) || $value < 0 || $value > 0xFFFF_FFFF) {
            throw new ProtocolException($label . ' must be a uint32');
        }
        return $value;
    }

    /** @param array<string,mixed> $node */
    public static function finiteScalarField(array $node, string $key, string $label, float $default = 0.0): float
    {
        if (!array_key_exists($key, $node)) {
            return $default;
        }
        return self::finiteNumber($node[$key], $label);
    }

    /** @internal
     *  @param array<string,mixed> $node
     *  @return list<mixed>
     */
    public static function listField(array $node, string $key, string $label): array
    {
        if (!array_key_exists($key, $node)) {
            return [];
        }
        $value = $node[$key];
        if (!is_array($value) || !array_is_list($value)) {
            throw new ProtocolException($label . ' must be a list');
        }
        return $value;
    }

    public static function encodeJson(array $value): string
    {
        return self::encodeJsonNode($value);
    }

    private static function encodeJsonNode(mixed $value): string
    {
        if ($value instanceof JsonObject) {
            return self::encodeJsonObject($value->fields);
        }

        if (is_array($value)) {
            if (array_is_list($value)) {
                return '[' . implode(',', array_map(self::encodeJsonNode(...), $value)) . ']';
            }

            return self::encodeJsonObject($value);
        }

        if (is_object($value)) {
            throw new ProtocolException('unexpected object in standard FlatBuffers JSON tree');
        }

        try {
            return json_encode(
                $value,
                JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION | JSON_UNESCAPED_UNICODE,
            );
        } catch (\JsonException $error) {
            throw new ProtocolException('encode Search Kernel request JSON', 0, $error);
        }
    }

    /** @param array<string,mixed> $fields */
    private static function encodeJsonObject(array $fields): string
    {
        $parts = [];
        foreach ($fields as $key => $item) {
            if (!is_string($key)) {
                throw new ProtocolException('standard FlatBuffers JSON object keys must be strings');
            }
            $encoded = self::encodeJsonNode($item);
            try {
                $encodedKey = json_encode($key, JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE);
            } catch (\JsonException $error) {
                throw new ProtocolException('encode Search Kernel JSON object key', 0, $error);
            }
            $parts[] = $encodedKey . ':' . $encoded;
        }
        return '{' . implode(',', $parts) . '}';
    }

    /** @param array<string,mixed> $payload */
    private static function emptyMarker(array $payload, string $label): float
    {
        if ($payload !== []) {
            throw new ProtocolException($label . ' payload must be empty');
        }
        return INF;
    }

    /** @param array<string, mixed> $payload */
    private static function required(array $payload, string $key): mixed
    {
        if (!array_key_exists($key, $payload)) {
            throw new ProtocolException('missing portable union field: ' . $key);
        }
        return $payload[$key];
    }
}
