<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;
use UnexpectedValueException;

final class Index
{
    public function __construct(private readonly Transport $transport) {}

    public static function connect(string $baseUrl): self
    {
        return new self(new HttpTransport($baseUrl));
    }

    /** @param array<string, mixed> $document */
    public function insert(array $document): string
    {
        $payload = $this->execute(
            'InsertRequest',
            ['document' => Protocol::document($document)],
            'InsertResult',
        );
        return (string) ($payload['id'] ?? '');
    }

    /** @param array<string, mixed> $document */
    public function upsert(array $document, ?string $condition = null): string
    {
        $request = ['document' => Protocol::document($document)];
        if ($condition !== null) {
            $request['condition'] = $condition;
        }
        $payload = $this->execute('UpsertRequest', $request, 'UpsertResult');
        return (string) ($payload['id'] ?? '');
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
        $payload = $this->execute('GetRequest', $request, 'GetResult');
        $document = $payload['document'] ?? null;
        if ($document === null) {
            return null;
        }
        if (!is_array($document)) {
            throw new UnexpectedValueException('GetResult.document must be an object or null');
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
        $payload = $this->execute('DeleteRequest', $request, 'DeleteResult');
        return (bool) ($payload['deleted'] ?? false);
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

    /** @return array<string,mixed> */
    public function describe(): array
    {
        $payload = $this->execute('DescribeRequest', [], 'DescribeResult');
        $index = $payload['index'] ?? null;
        if (!is_array($index)) {
            throw new UnexpectedValueException('DescribeResult.index must be an object');
        }
        return $index;
    }

    /** @internal
     *  @param list<array<string,mixed>> $operations
     */
    public function executePatch(string $id, array $operations): string
    {
        $payload = $this->execute(
            'PatchRequest',
            ['id' => $id, 'operations' => $operations],
            'PatchResult',
        );
        return (string) ($payload['id'] ?? '');
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
        $payload = $this->execute('UpdateRequest', $request, 'UpdateResult');
        return (string) ($payload['id'] ?? '');
    }

    /** @internal
     *  @param array<string,mixed> $request
     */
    public function executeSearchRequest(array $request): SearchResult
    {
        $payload = $this->execute('SearchRequest', $request, 'SearchResult');
        return SearchResult::fromPayload($payload);
    }

    /** @param array<string, mixed> $payload
     *  @return array<string, mixed>
     */
    private function execute(string $requestType, array $payload, string $responseType): array
    {
        $response = $this->transport->execute(Protocol::envelope($requestType, $payload));
        return Protocol::expectPayload($response, $responseType);
    }

    private static function id(string $id): void
    {
        if ($id === '') {
            throw new InvalidArgumentException('document id must be a non-empty string');
        }
    }
}
