<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class AnalyzeBuilder
{
    /** @var array<string,mixed>|null */
    private ?array $target = null;
    private ?string $language = null;

    public function __construct(
        private readonly Index $index,
        private readonly string $text,
    ) {
        if ($text === '') {
            throw new InvalidArgumentException('analyze text must not be empty');
        }
    }

    public function field(string $field): self
    {
        if ($field === '') {
            throw new InvalidArgumentException('analyze field must not be empty');
        }
        $copy = clone $this;
        $copy->target = [
            'target_type' => 'AnalyzeFieldTarget',
            'target' => ['field' => $field],
        ];
        return $copy;
    }

    /** @param array<string,mixed> $options */
    public function analyzer(string $name, int $version = 0, array $options = []): self
    {
        if ($name === '') {
            throw new InvalidArgumentException('analyzer name must not be empty');
        }
        if ($version < 0 || $version > 0xFFFF_FFFF) {
            throw new InvalidArgumentException('analyzer version must be a uint32');
        }
        $copy = clone $this;
        $copy->target = [
            'target_type' => 'Analyzer',
            'target' => [
                'name' => $name,
                'version' => $version,
                'options' => Protocol::valueEntries($options),
            ],
        ];
        return $copy;
    }

    public function language(string $language): self
    {
        if ($language === '') {
            throw new InvalidArgumentException('analysis language must not be empty');
        }
        $copy = clone $this;
        $copy->language = $language;
        return $copy;
    }

    /** @return list<array{surface:string,term:string,position:int,start_byte:int,end_byte:int}> */
    public function execute(): array
    {
        $request = ['text' => $this->text];
        if ($this->target !== null) {
            $request['target'] = $this->target;
        }
        if ($this->language !== null) {
            $request['language'] = $this->language;
        }

        $payload = $this->index->executeAnalyzeRequest($request);
        $tokens = $payload['tokens'] ?? null;
        if (!is_array($tokens) || !array_is_list($tokens)) {
            throw new ProtocolException('AnalyzeResult.tokens must be a list');
        }

        $decoded = [];
        foreach ($tokens as $token) {
            if (!is_array($token)
                || !is_string($token['surface'] ?? null)
                || !is_string($token['term'] ?? null)
                || !is_int($token['position'] ?? null)
                || !is_int($token['start_byte'] ?? null)
                || !is_int($token['end_byte'] ?? null)
                || $token['position'] < 0
                || $token['start_byte'] < 0
                || $token['end_byte'] < 0
                || $token['position'] > 0xFFFF_FFFF
                || $token['start_byte'] > 0xFFFF_FFFF
                || $token['end_byte'] > 0xFFFF_FFFF
            ) {
                throw new ProtocolException('malformed AnalyzeResult token');
            }
            $decoded[] = [
                'surface' => $token['surface'],
                'term' => $token['term'],
                'position' => $token['position'],
                'start_byte' => $token['start_byte'],
                'end_byte' => $token['end_byte'],
            ];
        }

        return $decoded;
    }
}
