<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Index
{
    private readonly Client $client;

    public function __construct(Transport $transport, ?string $target = null)
    {
        $this->client = new Client($transport, $target);
        if ($target !== null) {
            $this->client->requireOpenBeforeOperations();
        }
    }

    /** Close this client handle; shared server indexes remain available. */
    public function close(): void
    {
        $this->client->close();
    }

    public static function connect(string $baseUrl, string $target): self
    {
        $index = new self(new HttpTransport($baseUrl), $target);
        $payload = $index->client->openIndex();
        self::lifecycleIndex($payload, 'OpenIndexResult');
        $index->client->bindGeneration($payload['generation'] ?? null);
        return $index;
    }

    public static function create(
        string $baseUrl,
        string $target,
        IndexDefinition $definition,
    ): self {
        $index = new self(new HttpTransport($baseUrl), $target);
        $payload = $index->client->createIndex(['definition' => $definition->toWire()]);
        self::lifecycleIndex($payload, 'CreateIndexResult');
        $index->client->bindGeneration($payload['generation'] ?? null);
        return $index;
    }

    public static function open(string $baseUrl, string $target): self
    {
        return self::connect($baseUrl, $target);
    }

    public static function deleteTarget(string $baseUrl, string $target): bool
    {
        $client = Client::connect($baseUrl, target: $target);
        try {
            $client->deleteIndex();
            return true;
        } finally {
            $client->close();
        }
    }

    /** @param array<string, mixed> $document */
    public function insert(array $document): string
    {
        $payload = $this->client->insert(['document' => Protocol::document($document)]);
        return self::resultId($payload, 'InsertResult');
    }

    /** @param array<string, mixed> $document */
    public function upsert(array $document, ?string $condition = null): string
    {
        $request = ['document' => Protocol::document($document)];
        if ($condition !== null) {
            $request['condition'] = $condition;
        }
        $payload = $this->client->upsert($request);
        return self::resultId($payload, 'UpsertResult');
    }

    /**
     * @param list<string>|null $fields
     * @return array<string, mixed>|null
     */
    public function get(string $id, ?array $fields = null): ?array
    {
        self::id($id);
        $request = ['id' => $id];
        if ($fields !== null) {
            $request['projection'] = ['fields' => array_values($fields)];
        }
        $payload = $this->client->get($request);
        $document = $payload['document'] ?? null;
        if ($document === null) {
            return null;
        }
        if (!is_array($document)) {
            throw new ProtocolException('GetResult.document must be an object or null');
        }
        return Protocol::decodeDocument($document);
    }

    public function delete(string $id, ?string $condition = null): bool
    {
        self::id($id);
        $request = ['id' => $id];
        if ($condition !== null) {
            $request['condition'] = $condition;
        }
        $payload = $this->client->delete($request);
        self::resultId($payload, 'DeleteResult');
        return Protocol::boolScalarField(
            $payload,
            'deleted',
            'DeleteResult.deleted',
            false,
        );
    }

    public function patch(string $id): PatchBuilder
    {
        self::id($id);
        return new PatchBuilder($this, $id);
    }

    public function update(string $id): UpdateBuilder
    {
        self::id($id);
        return new UpdateBuilder($this, $id);
    }

    public function search(string $text): SearchBuilder
    {
        if (trim($text) === '') {
            throw new InvalidArgumentException('search text must not be empty; use browse() for match-all');
        }
        return new SearchBuilder($this, [
            'source_type' => 'SearchKernelTextQuery',
            'source' => ['text' => $text],
        ]);
    }

    public function browse(): SearchBuilder
    {
        return new SearchBuilder($this, [
            'source_type' => 'MatchAllQuery',
            'source' => [],
        ]);
    }

    /** @param array<string,mixed> $query */
    public function query(array $query): SearchBuilder
    {
        if (($query['source_type'] ?? null) !== 'TypedQuery') {
            throw new InvalidArgumentException('query() expects a Query helper result');
        }
        return new SearchBuilder($this, $query);
    }

    /** @param array<string,mixed> $parameters */
    public function yql(string $source, array $parameters = []): YqlBuilder
    {
        if (trim($source) === '') {
            throw new InvalidArgumentException('YQL source must not be empty');
        }
        $query = [
            'source_type' => 'YqlQuery',
            'source' => ['source' => $source],
        ];
        if ($parameters !== []) {
            $query['source']['parameters'] = Protocol::valueEntries($parameters);
        }
        return new YqlBuilder($this, $query);
    }

    public function analyze(string $text): AnalyzeBuilder
    {
        return new AnalyzeBuilder($this, $text);
    }

    /** @return array<string,mixed> */
    public function describe(): array
    {
        $payload = $this->client->describe();
        $index = $payload['index'] ?? null;
        if (!is_array($index)) {
            throw new ProtocolException('DescribeResult.index must be an object');
        }
        return $index;
    }

    /** @internal
     *  @param list<array<string,mixed>> $operations
     */
    public function executePatch(string $id, array $operations): string
    {
        $payload = $this->client->patch(['id' => $id, 'operations' => $operations]);
        return self::resultId($payload, 'PatchResult');
    }

    /** @internal
     *  @param list<array<string,mixed>> $operations
     */
    public function executeUpdate(
        string $id,
        array $operations,
        ?string $condition,
        ?bool $create,
    ): string {
        $request = ['id' => $id, 'operations' => $operations];
        if ($condition !== null) {
            $request['condition'] = $condition;
        }
        if ($create !== null) {
            $request['create'] = ['value' => $create];
        }
        $payload = $this->client->update($request);
        return self::resultId($payload, 'UpdateResult');
    }

    /** @internal
     *  @param array<string,mixed> $request
     *  @return array<string,mixed>
     */
    public function executeAnalyzeRequest(array $request): array
    {
        return $this->client->analyze($request);
    }

    /** @internal
     *  @param array<string,mixed> $request
     *  @return array<string,mixed>
     */
    public function inspectSearchRequest(array $request): array
    {
        $payload = $this->client->inspectQuery(['request' => $request]);
        $resolved = $payload['resolved_request'] ?? null;
        if (!is_array($resolved)) {
            throw new ProtocolException('InspectQueryResult.resolved_request must be an object');
        }
        return $resolved;
    }

    /** @internal
     *  @param array<string,mixed> $request
     */
    public function executeSearchRequest(array $request): SearchResult
    {
        $payload = $this->client->search($request);
        return SearchResult::fromPayload($payload);
    }

    /** @param array<string,mixed> $payload */
    private static function lifecycleIndex(array $payload, string $resultType): array
    {
        $index = $payload['index'] ?? null;
        if (!is_array($index)) {
            throw new ProtocolException($resultType . '.index must be an object');
        }
        return $index;
    }

    /** @param array<string,mixed> $payload */
    private static function resultId(array $payload, string $resultType): string
    {
        $id = $payload['id'] ?? null;
        if (!is_string($id) || $id === '') {
            throw new ProtocolException($resultType . '.id must be a non-empty string');
        }
        return $id;
    }

    private static function id(string $id): void
    {
        if ($id === '') {
            throw new InvalidArgumentException('document id must be a non-empty string');
        }
    }
}
