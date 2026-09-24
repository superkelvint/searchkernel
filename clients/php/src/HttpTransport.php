<?php

declare(strict_types=1);

namespace SearchKernel;

use JsonException;

final class HttpTransport implements Transport
{
    private string $executeUrl;

    public function __construct(string $baseUrl, private readonly float $timeoutSeconds = 30.0)
    {
        if (!is_finite($timeoutSeconds) || $timeoutSeconds <= 0.0) {
            throw new TransportException('Search Kernel HTTP timeout must be a positive finite number');
        }
        $baseUrl = rtrim($baseUrl, '/');
        if ($baseUrl === '') {
            throw new TransportException('Search Kernel HTTP base URL must not be empty');
        }
        $this->executeUrl = $baseUrl . '/v1/execute';
    }

    /** @param array<string, mixed> $request
     *  @return array<string, mixed>
     */
    public function execute(array $request): array
    {
        $body = Protocol::encodeJson($request);

        $context = stream_context_create([
            'http' => [
                'method' => 'POST',
                'header' => "Content-Type: application/json\r\nAccept: application/json\r\n",
                'content' => $body,
                'timeout' => $this->timeoutSeconds,
                'ignore_errors' => true,
                'follow_location' => 0,
                'max_redirects' => 0,
            ],
        ]);

        $response = @file_get_contents($this->executeUrl, false, $context);
        $headers = $http_response_header ?? [];
        $status = self::statusCode($headers);

        if ($response === false && $status === 0) {
            $last = error_get_last();
            throw new TransportException(
                'Search Kernel HTTP request failed' . ($last ? ': ' . $last['message'] : '')
            );
        }
        if ($status < 200 || $status >= 300) {
            throw new TransportException(sprintf(
                'Search Kernel HTTP request failed with status %d: %s',
                $status,
                is_string($response) ? trim($response) : '',
            ));
        }

        try {
            $decoded = json_decode(
                is_string($response) ? $response : '',
                true,
                512,
                JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING,
            );
        } catch (JsonException $error) {
            throw new ProtocolException('decode Search Kernel response JSON', 0, $error);
        }

        if (!is_array($decoded) || array_is_list($decoded)) {
            throw new ProtocolException('Search Kernel response JSON must be an object');
        }
        return $decoded;
    }

    /** @param list<string> $headers */
    private static function statusCode(array $headers): int
    {
        $status = 0;
        foreach ($headers as $header) {
            if (preg_match('/^HTTP\/\S+\s+(\d{3})\b/', $header, $matches) === 1) {
                $status = (int) $matches[1];
            }
        }
        return $status;
    }
}
