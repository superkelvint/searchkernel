<?php

declare(strict_types=1);

namespace SearchKernel;

final class Index
{
    public function __construct(private readonly Client $client) {}

    public static function connect(string $baseUrl): self
    {
        return new self(Client::connect($baseUrl));
    }

    /** @param array<string, mixed> $document */
    public function insert(array $document): string
    {
        $payload = $this->execute(
            'insert',
            ['document' => Protocol::document($document)],
        );
        return (string) ($payload['id'] ?? '');
    }

    /** @param array<string, mixed> $document */
    public function upsert(array $document): string
    {
        $payload = $this->execute(
            'upsert',
            ['document' => Protocol::document($document)],
        );
        return (string) ($payload['id'] ?? '');
    }

    /** @return array<string, mixed>|null */
    public function get(string $id): ?array
    {
        $payload = $this->execute('get', ['id' => $id]);
        $document = $payload['document'] ?? null;
        if ($document === null) {
            return null;
        }
        if (!is_array($document)) {
            throw new ProtocolException('GetResult.document must be an object or null');
        }
        return Protocol::decodeDocument($document);
    }

    public function delete(string $id): bool
    {
        $payload = $this->execute('delete', ['id' => $id]);
        return (bool) ($payload['deleted'] ?? false);
    }

    public function search(string $text): SearchBuilder
    {
        return new SearchBuilder($this, $text);
    }

    /** @internal */
    public function executeSearch(string $text, ?int $limit): SearchResult
    {
        $request = [
            'query' => [
                'source_type' => 'SearchKernelTextQuery',
                'source' => ['text' => $text],
            ],
        ];
        if ($limit !== null) {
            $request['limit'] = ['value' => $limit];
        }

        $payload = $this->execute('search', $request);
        return SearchResult::fromPayload($payload);
    }

    /** @param array<string, mixed> $payload
     *  @return array<string, mixed>
     */
    private function execute(string $operation, array $payload): array
    {
        return $this->client->execute($operation, $payload);
    }
}
