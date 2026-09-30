<?php

declare(strict_types=1);

use SearchKernel\Field;
use SearchKernel\IndexDefinition;
use SearchKernel\RankProfile;

function searchkernelPhpClientApiE2eCorpus(): array
{
    $path = dirname(__DIR__, 3) . '/reference/client-real-e2e-parity/corpus.json';
    return json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);
}

function searchkernelPhpClientApiE2eDefinition(array $corpus): IndexDefinition
{
    $fields = [];
    foreach ($corpus['schema']['fields'] as $name => $field) {
        $fields[$name] = searchkernelPhpClientApiE2eField($field);
    }

    $profiles = [];
    foreach ($corpus['schema']['rank_profiles'] as $spec) {
        $profile = new RankProfile($spec['name']);
        if (isset($spec['first_phase'])) $profile->firstPhase($spec['first_phase']);
        if (isset($spec['second_phase'])) $profile->secondPhase($spec['second_phase']);
        if (isset($spec['rerank_count'])) $profile->rerankCount($spec['rerank_count']);
        foreach ($spec['match_features'] ?? [] as $feature) $profile->matchFeature($feature);
        foreach ($spec['query_inputs'] ?? [] as $name => $type) $profile->queryInput($name, $type);
        $profiles[] = $profile;
    }
    return new IndexDefinition(
        fields: $fields,
        rankProfiles: $profiles,
    );
}

/** @param array<string, mixed> $document @return array<string, mixed> */
function searchkernelPhpClientApiE2eDocument(array $corpus, array $document): array
{
    foreach ($document as $name => $value) {
        $field = $corpus['schema']['fields'][$name] ?? null;
        if (($field['type'] ?? null) === 'float64' && is_int($value)) {
            $document[$name] = (float) $value;
        }
    }
    return $document;
}

function searchkernelPhpClientApiE2eQueryValue(array $corpus, string $field, mixed $value): mixed
{
    if (($corpus['schema']['fields'][$field]['type'] ?? null) === 'float64' && is_int($value)) {
        return (float) $value;
    }
    return $value;
}

function searchkernelPhpClientApiE2eField(array $spec): Field
{
    $capabilities = array_intersect_key($spec, array_flip(['stored', 'searchable', 'filterable', 'sortable', 'facetable']));
    return match ($spec['type']) {
        'text' => Field::text(...$capabilities),
        'keyword' => Field::keyword(...$capabilities),
        'float64' => Field::float64(...$capabilities),
        'int64' => Field::int64(...$capabilities),
        'bool' => Field::boolean(...$capabilities),
        'position' => Field::position(...$capabilities),
        'array' => Field::arrayOf(
            searchkernelPhpClientApiE2eField($spec['element']),
            ...$capabilities,
        ),
        'struct' => Field::structOf(array_map(
            static fn (array $member): Field => searchkernelPhpClientApiE2eField($member),
            $spec['fields'],
        )),
        default => str_starts_with($spec['type'], 'tensor<')
            ? Field::tensor($spec['type'], ...$capabilities)
            : throw new InvalidArgumentException("unsupported parity corpus field type: {$spec['type']}"),
    };
}
