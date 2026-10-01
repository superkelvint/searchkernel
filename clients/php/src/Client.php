<?php

declare(strict_types=1);

namespace SearchKernel;

/**
 * Low-level portable client over canonical standard FlatBuffers JSON.
 *
 * Request/response payload arrays are table bodies from the canonical schema
 * schema. This layer owns operation-to-union mapping only; Search Kernel
 * semantics remain server-owned.
 */
final class Client
{
    private bool $closed = false;
    private ?string $indexGeneration = null;

    public function __construct(
        private readonly Transport $transport,
        private readonly ?string $target = null,
    ) {}

    public static function connect(
        string $baseUrl,
        float $timeoutSeconds = 30.0,
        ?string $target = null,
    ): self {
        return new self(new HttpTransport($baseUrl, $timeoutSeconds), $target);
    }

    /**
     * @param array<string,mixed>|JsonObject $payload
     * @return array<string,mixed>
     */
    public function execute(string $operation, array|JsonObject $payload): array
    {
        if ($this->closed) {
            throw new TransportException('Search Kernel client is closed');
        }
        [$requestType, $responseType] = Protocol::operation($operation);
        $lifecycle = in_array(
            $operation,
            ['create_index', 'open_index', 'delete_index'],
            true,
        );
        $response = $this->transport->execute(
            Protocol::envelope(
                $requestType,
                $payload,
                $this->target,
                $lifecycle ? null : $this->indexGeneration,
            ),
        );
        $result = Protocol::expectPayload($response, $responseType);
        if ($operation === 'create_index' || $operation === 'open_index') {
            $generation = $result['generation'] ?? null;
            if (!is_string($generation) || $generation === '') {
                throw new ProtocolException(
                    $responseType . ' is missing canonical index generation',
                );
            }
            $this->indexGeneration = $generation;
        }
        return $result;
    }

    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function insert(array $payload): array { return $this->execute('insert', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function upsert(array $payload): array { return $this->execute('upsert', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function patch(array $payload): array { return $this->execute('patch', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function update(array $payload): array { return $this->execute('update', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function delete(array $payload): array { return $this->execute('delete', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function get(array $payload): array { return $this->execute('get', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function search(array $payload): array { return $this->execute('search', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function analyze(array $payload): array { return $this->execute('analyze', $payload); }
    /** @return array<string,mixed> */
    public function describe(): array { return $this->execute('describe', []); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function inspectQuery(array $payload): array { return $this->execute('inspect_query', $payload); }
    /** @param array<string,mixed> $payload @return array<string,mixed> */
    public function createIndex(array $payload): array { return $this->execute('create_index', $payload); }
    /** @return array<string,mixed> */
    public function openIndex(): array { return $this->execute('open_index', JsonObject::empty()); }
    /** @return array<string,mixed> */
    public function deleteIndex(): array { return $this->execute('delete_index', JsonObject::empty()); }

    /** Close this PHP handle without changing shared daemon index state. */
    public function close(): void
    {
        $this->closed = true;
    }
}
