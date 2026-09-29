<?php

declare(strict_types=1);

use SearchKernel\Field;
use SearchKernel\IndexDefinition;
use SearchKernel\RankProfile;

function searchkernelPhpClientApiE2eDefinition(string $vectorType): IndexDefinition
{
    return new IndexDefinition(
        fields: [
            'title' => Field::text(stored: true, searchable: true),
            'brand' => Field::keyword(stored: true, filterable: true, sortable: true, facetable: true),
            'category' => Field::keyword(stored: true, filterable: true, facetable: true),
            'price' => Field::float64(stored: true, filterable: true, sortable: true),
            'sale_price' => Field::float64(stored: true, filterable: true, sortable: true),
            'rating' => Field::int64(stored: true, filterable: true, sortable: true),
            'inventory' => Field::int64(stored: true, filterable: true, sortable: true),
            'in_stock' => Field::boolean(stored: true, filterable: true),
            'embedding' => Field::tensor($vectorType, stored: true),
            'people' => Field::arrayOf(
                Field::structOf([
                    'first' => Field::keyword(),
                    'last' => Field::keyword(),
                ]),
                stored: false,
                searchable: false,
            ),
            'location' => Field::position(stored: false),
        ],
        rankProfiles: [
            (new RankProfile('boosted'))
                ->firstPhase('query(boost)')
                ->matchFeature('query(boost)'),
            (new RankProfile('semantic'))
                ->firstPhase('closeness(embedding)')
                ->queryInput('query_embedding', $vectorType)
                ->matchFeature('query(query_embedding)'),
            (new RankProfile('hybrid'))
                ->firstPhase('bm25(title) + 10 * closeness(embedding)')
                ->queryInput('query_embedding', $vectorType)
                ->matchFeature('bm25(title)')
                ->matchFeature('closeness(embedding)')
                ->matchFeature('query(query_embedding)'),
            (new RankProfile('two_phase'))
                ->firstPhase('attribute(rating)')
                ->secondPhase('attribute(inventory)')
                ->rerankCount(3),
            (new RankProfile('first_phase_only'))
                ->firstPhase('attribute(rating)'),
        ],
    );
}
