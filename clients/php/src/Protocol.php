<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Protocol
{
    public const VERSION = 2;

    /** @var array<string, array{0:string,1:string}> */
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
    public static function envelope(string $payloadType, array $payload): array
    {
        return [
            'protocol_version' => self::VERSION,
            'payload_type' => 'SearchKernel_V2_' . $payloadType,
            'payload' => $payload,
        ];
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
        if (is_int($value)) {
            return [
                'kind_type' => 'Int64Value',
                'kind' => ['value' => $value],
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

        if ($type === 'SearchKernel_V2_Error') {
            throw SearchKernelException::fromPayload($payload);
        }

        if ($type !== 'SearchKernel_V2_' . $expectedType) {
            throw new ProtocolException(sprintf(
                'unexpected Search Kernel response type %s (expected %s)',
                is_string($type) ? $type : '<missing>',
                'SearchKernel_V2_' . $expectedType,
            ));
        }

        return $payload;
    }

    /** @param array<string, mixed> $document
     *  @return array<string, mixed>
     */
    public static function decodeDocument(array $document): array
    {
        $id = $document['id'] ?? null;
        if (!is_string($id) || $id === '') {
            throw new ProtocolException('Document.id must be a non-empty string');
        }
        $fields = self::decodeValueEntries($document['fields'] ?? []);
        $fields['_id'] = $id;
        return $fields;
    }

    /** @param mixed $entries
     *  @return array<string, mixed>
     */
    public static function decodeValueEntries(mixed $entries): array
    {
        if ($entries === null) {
            return [];
        }
        if (!is_array($entries)) {
            throw new ProtocolException('portable value entries must be an array');
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
            'NullValueValue' => null,
            'BoolValue' => (bool) self::required($payload, 'value'),
            'Int64Value', 'UInt64Value' => self::required($payload, 'value'),
            'Float64Value' => (float) self::required($payload, 'value'),
            'StringValue' => (string) self::required($payload, 'value'),
            'ValueList' => self::decodeValueList($payload),
            'ValueObject' => self::decodeValueEntries($payload['fields'] ?? []),
            default => throw new ProtocolException(
                'unsupported portable Value kind in first PHP slice: ' . $type
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
            'FiniteRelevance' => (float) self::required($payload, 'value'),
            'PositiveInfinityRelevance' => INF,
            'NegativeInfinityRelevance' => -INF,
            default => throw new ProtocolException('unsupported relevance kind: ' . $type),
        };
    }

    /** @param array<string, mixed> $payload
     *  @return list<mixed>
     */
    private static function decodeValueList(array $payload): array
    {
        $values = $payload['values'] ?? [];
        if (!is_array($values)) {
            throw new ProtocolException('portable ValueList values must be an array');
        }
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

    /** @param array<string, mixed> $payload */
    private static function required(array $payload, string $key): mixed
    {
        if (!array_key_exists($key, $payload)) {
            throw new ProtocolException('missing portable union field: ' . $key);
        }
        return $payload[$key];
    }
}
