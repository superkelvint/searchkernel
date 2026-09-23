<?php

declare(strict_types=1);

namespace SearchKernel;

/**
 * Low-level portable operation client.
 *
 * Payload arrays are standard FlatBuffers JSON table bodies from the canonical
 * SearchKernel.V2 schema. This layer maps operation identity only; it does not
 * validate or normalize Search Kernel semantics.
 */
final class Client
{
    public function __construct(private readonly Transport $transport) {}

    public static function connect(string $baseUrl): self
    {
        return new self(new HttpTransport($baseUrl));
    }

    /**
     * Execute one canonical portable operation.
     *
     * @param array<string, mixed> $payload standard FlatBuffers JSON table body
     * @return array<string, mixed> standard FlatBuffers JSON result table body
     */
    public function execute(string $operation, array $payload): array
    {
        [$requestType, $responseType] = Protocol::operation($operation);
        $response = $this->transport->execute(
            Protocol::envelope($requestType, $payload),
        );
        return Protocol::expectPayload($response, $responseType);
    }

    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function insert(array $payload): array { return $this->execute('insert', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function upsert(array $payload): array { return $this->execute('upsert', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function patch(array $payload): array { return $this->execute('patch', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function update(array $payload): array { return $this->execute('update', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function delete(array $payload): array { return $this->execute('delete', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function get(array $payload): array { return $this->execute('get', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function search(array $payload): array { return $this->execute('search', $payload); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function analyze(array $payload): array { return $this->execute('analyze', $payload); }
    /** @return array<string, mixed> */
    public function describe(): array { return $this->execute('describe', []); }
    /** @param array<string, mixed> $payload @return array<string, mixed> */
    public function inspectQuery(array $payload): array { return $this->execute('inspect_query', $payload); }
}
