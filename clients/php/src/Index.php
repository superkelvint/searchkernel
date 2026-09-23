<?php

declare(strict_types=1);

namespace SearchKernel;

final class Index
{
    public function __construct(private readonly HttpTransport $transport) {}

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
    public function upsert(array $document): string
    {
        $payload = $this->execute(
            'UpsertRequest',
            ['document' => Protocol::document($document)],
            'UpsertResult',
        );
        return (string) ($payload['id'] ?? '');
    }

    /** @return array<string, mixed>|null */
    public function get(string $id): ?array
    {
        $payload = $this->execute('GetRequest', ['id' => $id], 'GetResult');
        $document = $payload['document'] ?? null;
        if ($document === null) {
            return null;
        }
        if (!is_array($document)) {
            throw new \UnexpectedValueException('GetResult.document must be an object or null');
        }
        return Protocol::decodeDocument($document);
    }

    public function delete(string $id): bool
    {
        $payload = $this->execute('DeleteRequest', ['id' => $id], 'DeleteResult');
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
}
