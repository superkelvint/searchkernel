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
        $capabilities = array_intersect_key($field, array_flip(['stored', 'searchable', 'filterable', 'sortable', 'facetable']));
        $fields[$name] = match (true) {
            $field['type'] === 'text' => Field::text(...$capabilities),
            $field['type'] === 'keyword' => Field::keyword(...$capabilities),
            $field['type'] === 'float64' => Field::float64(...$capabilities),
            $field['type'] === 'int64' => Field::int64(...$capabilities),
            $field['type'] === 'bool' => Field::boolean(...$capabilities),
            str_starts_with($field['type'], 'tensor<') => Field::tensor($field['type'], ...$capabilities),
            default => throw new InvalidArgumentException("unsupported parity corpus field type: {$field['type']}"),
        };
    }
    // These fields extend the shared parity schema for the legacy PHP-only
    // typed-query showcase. They are deliberately added after corpus fields.
    $fields['people'] = Field::arrayOf(
        Field::structOf(['first' => Field::keyword(), 'last' => Field::keyword()]),
        stored: false,
        searchable: false,
    );
    $fields['location'] = Field::position(stored: false);

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
