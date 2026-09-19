use super::*;

pub(super) fn query_source_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    query: &ir::QueryIr,
) -> flatbuffers::WIPOffset<api::QuerySource<'a>> {
    let (source_type, source) = match query {
        ir::QueryIr::Text(text) => {
            let text = builder.create_string(text);
            let value = api::SearchKernelTextQuery::create(
                builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            (
                api::QuerySourceKind::SearchKernelTextQuery,
                value.as_union_value(),
            )
        }
        ir::QueryIr::MatchAll => {
            let value = api::MatchAllQuery::create(builder, &api::MatchAllQueryArgs {});
            (api::QuerySourceKind::MatchAllQuery, value.as_union_value())
        }
        ir::QueryIr::Yql { source, parameters } => {
            let source = builder.create_string(source);
            let parameters = request_value_entries_offset(builder, parameters);
            let value = api::YqlQuery::create(
                builder,
                &api::YqlQueryArgs {
                    source: Some(source),
                    parameters,
                },
            );
            (api::QuerySourceKind::YqlQuery, value.as_union_value())
        }
        _ => {
            let value = query_expr_offset(builder, query);
            let value =
                api::TypedQuery::create(builder, &api::TypedQueryArgs { query: Some(value) });
            (api::QuerySourceKind::TypedQuery, value.as_union_value())
        }
    };
    api::QuerySource::create(
        builder,
        &api::QuerySourceArgs {
            source_type,
            source: Some(source),
        },
    )
}

pub(super) fn search_field_selection_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::SearchFieldIr],
) -> flatbuffers::WIPOffset<api::SearchFieldSelection<'a>> {
    let fields = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(field.field.as_str());
            let weight = api::OptionalDouble::new(field.weight.get());
            api::SearchField::create(
                builder,
                &api::SearchFieldArgs {
                    field: Some(name),
                    weight: Some(&weight),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::SearchFieldSelection::create(
        builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(fields),
        },
    )
}

pub(super) fn projection_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    projection: &ir::ProjectionIr,
) -> flatbuffers::WIPOffset<api::FieldProjection<'a>> {
    let fields = projection
        .fields
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::FieldProjection::create(
        builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    )
}

pub(super) fn highlighting_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::FieldName],
) -> flatbuffers::WIPOffset<api::HighlightRequest<'a>> {
    let fields = fields
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::HighlightRequest::create(
        builder,
        &api::HighlightRequestArgs {
            fields: Some(fields),
        },
    )
}

pub(super) fn query_expr_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    query: &ir::QueryIr,
) -> flatbuffers::WIPOffset<api::QueryExpr<'a>> {
    if let ir::QueryIr::Typed(query) = query {
        return query_expr_ir_offset(builder, query);
    }
    let (expr_type, expr) = match query {
        ir::QueryIr::Term { field, value } => {
            let field = query_field_ref_offset(builder, field.as_str());
            let value = scalar_value_offset(builder, value);
            let term = api::TermQuery::create(builder, &api::TermQueryArgs { value: Some(value) });
            let item = api::QueryExpr::create(
                builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::TermQuery,
                    expr: Some(term.as_union_value()),
                    annotations: None,
                },
            );
            let contains = api::QueryContains::create(
                builder,
                &api::QueryContainsArgs {
                    field: Some(field),
                    item: Some(item),
                },
            );
            (api::QueryExprKind::QueryContains, contains.as_union_value())
        }
        ir::QueryIr::And(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| query_expr_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::QueryAnd::create(
                builder,
                &api::QueryAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::QueryExprKind::QueryAnd, value.as_union_value())
        }
        ir::QueryIr::Or(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| query_expr_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::QueryOr::create(
                builder,
                &api::QueryOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::QueryExprKind::QueryOr, value.as_union_value())
        }
        ir::QueryIr::Not(clause) => {
            let clause = query_expr_offset(builder, clause);
            let value = api::QueryNot::create(
                builder,
                &api::QueryNotArgs {
                    clause: Some(clause),
                },
            );
            (api::QueryExprKind::QueryNot, value.as_union_value())
        }
        ir::QueryIr::MatchAll | ir::QueryIr::Text(_) | ir::QueryIr::Yql { .. } => unreachable!(),
        ir::QueryIr::Typed(_) => unreachable!(),
    };
    api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type,
            expr: Some(expr),
            annotations: None,
        },
    )
}

pub(super) fn query_alternatives_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::QueryAlternativeIr],
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let alternatives = values
        .iter()
        .map(|alternative| {
            let query = query_expr_ir_offset(builder, &alternative.query);
            let exactness = alternative.exactness.map(api::OptionalDouble::new);
            api::QueryAlternative::create(
                builder,
                &api::QueryAlternativeArgs {
                    query: Some(query),
                    exactness: exactness.as_ref(),
                },
            )
        })
        .collect::<Vec<_>>();
    let alternatives = builder.create_vector(&alternatives);
    let value = api::QueryAlternatives::create(
        builder,
        &api::QueryAlternativesArgs {
            alternatives: Some(alternatives),
        },
    );
    (
        api::QueryExprKind::QueryAlternatives,
        value.as_union_value(),
    )
}

pub(super) fn query_rank_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    matching: &ir::QueryExprIr,
    ranking: &[ir::QueryExprIr],
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let matching = query_expr_ir_offset(builder, matching);
    let ranking = query_expr_ir_vector(builder, ranking);
    let value = api::QueryRank::create(
        builder,
        &api::QueryRankArgs {
            match_: Some(matching),
            rank: ranking,
        },
    );
    (api::QueryExprKind::QueryRank, value.as_union_value())
}

pub(super) fn query_geo_location_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &ir::QueryFieldRefIr,
    latitude: &f64,
    longitude: &f64,
    radius: &str,
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let field = query_field_ref_ir_offset(builder, field);
    let radius = builder.create_string(radius);
    let value = api::QueryGeoLocation::create(
        builder,
        &api::QueryGeoLocationArgs {
            field: Some(field),
            latitude: *latitude,
            longitude: *longitude,
            radius: Some(radius),
        },
    );
    (api::QueryExprKind::QueryGeoLocation, value.as_union_value())
}

pub(super) fn query_geo_bounding_box_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &ir::QueryFieldRefIr,
    south: &f64,
    west: &f64,
    north: &f64,
    east: &f64,
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let field = query_field_ref_ir_offset(builder, field);
    let value = api::QueryGeoBoundingBox::create(
        builder,
        &api::QueryGeoBoundingBoxArgs {
            field: Some(field),
            south: *south,
            west: *west,
            north: *north,
            east: *east,
        },
    );
    (
        api::QueryExprKind::QueryGeoBoundingBox,
        value.as_union_value(),
    )
}

pub(super) fn query_nearest_neighbor_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    document_field: &ir::QueryFieldRefIr,
    query_tensor: &str,
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let field = query_field_ref_ir_offset(builder, document_field);
    let query_tensor = builder.create_string(query_tensor);
    let value = api::QueryNearestNeighbor::create(
        builder,
        &api::QueryNearestNeighborArgs {
            document_field: Some(field),
            query_tensor: Some(query_tensor),
        },
    );
    (
        api::QueryExprKind::QueryNearestNeighbor,
        value.as_union_value(),
    )
}

pub(super) fn query_expr_ir_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    query: &ir::QueryExprIr,
) -> flatbuffers::WIPOffset<api::QueryExpr<'a>> {
    let (expr_type, expr) = match &query.kind {
        ir::QueryExprKindIr::Term { value } => {
            let value = request_value_offset(builder, value);
            let value = api::TermQuery::create(builder, &api::TermQueryArgs { value: Some(value) });
            (api::QueryExprKind::TermQuery, value.as_union_value())
        }
        ir::QueryExprKindIr::Contains { field, item } => {
            let field = query_field_ref_ir_offset(builder, field);
            let item = query_expr_ir_offset(builder, item);
            let value = api::QueryContains::create(
                builder,
                &api::QueryContainsArgs {
                    field: Some(field),
                    item: Some(item),
                },
            );
            (api::QueryExprKind::QueryContains, value.as_union_value())
        }
        ir::QueryExprKindIr::And(values) => {
            let clauses = query_expr_ir_vector(builder, values);
            let value = api::QueryAnd::create(builder, &api::QueryAndArgs { clauses });
            (api::QueryExprKind::QueryAnd, value.as_union_value())
        }
        ir::QueryExprKindIr::Or(values) => {
            let clauses = query_expr_ir_vector(builder, values);
            let value = api::QueryOr::create(builder, &api::QueryOrArgs { clauses });
            (api::QueryExprKind::QueryOr, value.as_union_value())
        }
        ir::QueryExprKindIr::Not(value) => {
            let clause = query_expr_ir_offset(builder, value);
            let value = api::QueryNot::create(
                builder,
                &api::QueryNotArgs {
                    clause: Some(clause),
                },
            );
            (api::QueryExprKind::QueryNot, value.as_union_value())
        }
        ir::QueryExprKindIr::True => {
            let value = api::QueryTrue::create(builder, &api::QueryTrueArgs {});
            (api::QueryExprKind::QueryTrue, value.as_union_value())
        }
        ir::QueryExprKindIr::False => {
            let value = api::QueryFalse::create(builder, &api::QueryFalseArgs {});
            (api::QueryExprKind::QueryFalse, value.as_union_value())
        }
        ir::QueryExprKindIr::Comparison { field, op, value } => {
            let field = query_field_ref_ir_offset(builder, field);
            let value = request_value_offset(builder, value);
            let value = api::QueryComparison::create(
                builder,
                &api::QueryComparisonArgs {
                    field: Some(field),
                    op: comparison_op_to_api(*op),
                    value: Some(value),
                },
            );
            (api::QueryExprKind::QueryComparison, value.as_union_value())
        }
        ir::QueryExprKindIr::Range {
            field,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            let field = query_field_ref_ir_offset(builder, field);
            let lower = query_range_bound_offset(builder, lower);
            let upper = query_range_bound_offset(builder, upper);
            let include_lower = include_lower.map(api::OptionalBool::new);
            let include_upper = include_upper.map(api::OptionalBool::new);
            let value = api::QueryRange::create(
                builder,
                &api::QueryRangeArgs {
                    field: Some(field),
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: include_lower.as_ref(),
                    include_upper: include_upper.as_ref(),
                },
            );
            (api::QueryExprKind::QueryRange, value.as_union_value())
        }
        ir::QueryExprKindIr::In { field, values } => {
            let field = query_field_ref_ir_offset(builder, field);
            let values = values
                .iter()
                .map(|value| request_value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let value = api::QueryIn::create(
                builder,
                &api::QueryInArgs {
                    field: Some(field),
                    values: Some(values),
                },
            );
            (api::QueryExprKind::QueryIn, value.as_union_value())
        }
        ir::QueryExprKindIr::Phrase(values) => {
            query_children_union(builder, values, api::QueryExprKind::QueryPhrase)
        }
        ir::QueryExprKindIr::Near { children, .. } => {
            query_children_union(builder, children, api::QueryExprKind::QueryNear)
        }
        ir::QueryExprKindIr::ONear { children, .. } => {
            query_children_union(builder, children, api::QueryExprKind::QueryONear)
        }
        ir::QueryExprKindIr::SameElement(values) => {
            query_children_union(builder, values, api::QueryExprKind::QuerySameElement)
        }
        ir::QueryExprKindIr::Equiv(values) => {
            query_children_union(builder, values, api::QueryExprKind::QueryEquiv)
        }
        ir::QueryExprKindIr::WeakAnd { children, .. } => {
            query_children_union(builder, children, api::QueryExprKind::QueryWeakAnd)
        }
        ir::QueryExprKindIr::Alternatives(values) => query_alternatives_union(builder, values),
        ir::QueryExprKindIr::Fuzzy {
            value: query_value, ..
        } => {
            let query_value = request_value_offset(builder, query_value);
            let value = api::QueryFuzzy::create(
                builder,
                &api::QueryFuzzyArgs {
                    value: Some(query_value),
                },
            );
            (api::QueryExprKind::QueryFuzzy, value.as_union_value())
        }
        ir::QueryExprKindIr::Matches { field, pattern } => {
            let field = query_field_ref_ir_offset(builder, field);
            let pattern = builder.create_string(pattern);
            let value = api::QueryMatches::create(
                builder,
                &api::QueryMatchesArgs {
                    field: Some(field),
                    pattern: Some(pattern),
                },
            );
            (api::QueryExprKind::QueryMatches, value.as_union_value())
        }
        ir::QueryExprKindIr::Text { text } => {
            let text = builder.create_string(text);
            let value = api::QueryText::create(builder, &api::QueryTextArgs { text: Some(text) });
            (api::QueryExprKind::QueryText, value.as_union_value())
        }
        ir::QueryExprKindIr::UserInput { value: input, .. } => {
            let input = builder.create_string(input);
            let value = api::QueryUserInput::create(
                builder,
                &api::QueryUserInputArgs { value: Some(input) },
            );
            (api::QueryExprKind::QueryUserInput, value.as_union_value())
        }
        ir::QueryExprKindIr::UserQuery { .. } => {
            let value = api::QueryUserQuery::create(builder, &api::QueryUserQueryArgs {});
            (api::QueryExprKind::QueryUserQuery, value.as_union_value())
        }
        ir::QueryExprKindIr::DotProduct { field, items } => {
            weighted_query_union(builder, field, items, api::QueryExprKind::QueryDotProduct)
        }
        ir::QueryExprKindIr::WeightedSet { field, items } => {
            weighted_query_union(builder, field, items, api::QueryExprKind::QueryWeightedSet)
        }
        ir::QueryExprKindIr::Wand { field, items } => {
            weighted_query_union(builder, field, items, api::QueryExprKind::QueryWand)
        }
        ir::QueryExprKindIr::Rank { matching, ranking } => {
            query_rank_union(builder, matching, ranking)
        }
        ir::QueryExprKindIr::Labeled {
            query: child,
            label,
            score,
        } => {
            let child = query_expr_ir_offset(builder, child);
            let label = builder.create_string(label);
            let value = api::QueryLabeled::create(
                builder,
                &api::QueryLabeledArgs {
                    query: Some(child),
                    label: Some(label),
                    score: *score,
                },
            );
            (api::QueryExprKind::QueryLabeled, value.as_union_value())
        }
        ir::QueryExprKindIr::NonEmpty(child) => {
            let child = query_expr_ir_offset(builder, child);
            let value =
                api::QueryNonEmpty::create(builder, &api::QueryNonEmptyArgs { query: Some(child) });
            (api::QueryExprKind::QueryNonEmpty, value.as_union_value())
        }
        ir::QueryExprKindIr::GeoLocation {
            field,
            latitude,
            longitude,
            radius,
        } => query_geo_location_union(builder, field, latitude, longitude, radius),
        ir::QueryExprKindIr::GeoBoundingBox {
            field,
            south,
            west,
            north,
            east,
        } => query_geo_bounding_box_union(builder, field, south, west, north, east),
        ir::QueryExprKindIr::NearestNeighbor {
            document_field,
            query_tensor,
        } => query_nearest_neighbor_union(builder, document_field, query_tensor),
        ir::QueryExprKindIr::Predicate {
            field,
            features,
            range_features,
        } => {
            let field = query_field_ref_ir_offset(builder, field);
            let features = value_object_offset(builder, features);
            let range_features = value_object_offset(builder, range_features);
            let value = api::QueryPredicate::create(
                builder,
                &api::QueryPredicateArgs {
                    field: Some(field),
                    features: Some(features),
                    range_features: Some(range_features),
                },
            );
            (api::QueryExprKind::QueryPredicate, value.as_union_value())
        }
    };
    let annotations = resolved_query_annotations(query);
    let annotations = request_value_entries_offset(builder, &annotations);
    api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type,
            expr: Some(expr),
            annotations,
        },
    )
}

fn resolved_query_annotations(
    query: &ir::QueryExprIr,
) -> std::collections::BTreeMap<String, ir::RequestValueIr> {
    let mut annotations = query.annotations.clone();
    match &query.kind {
        ir::QueryExprKindIr::Near { distance, .. }
        | ir::QueryExprKindIr::ONear { distance, .. } => {
            annotations.insert(
                "distance".to_owned(),
                ir::RequestValueIr::Int64(i64::from(*distance)),
            );
        }
        ir::QueryExprKindIr::WeakAnd { target_hits, .. } => {
            annotations.insert(
                "totalTargetHits".to_owned(),
                ir::RequestValueIr::Int64(i64::from(*target_hits)),
            );
        }
        ir::QueryExprKindIr::Fuzzy {
            max_edit_distance,
            prefix_length,
            prefix,
            ..
        } => {
            annotations.insert(
                "maxEditDistance".to_owned(),
                ir::RequestValueIr::Int64(i64::from(*max_edit_distance)),
            );
            annotations.insert(
                "prefixLength".to_owned(),
                ir::RequestValueIr::Int64(i64::from(*prefix_length)),
            );
            annotations.insert("prefix".to_owned(), ir::RequestValueIr::Bool(*prefix));
        }
        ir::QueryExprKindIr::UserInput {
            default_index: Some(default_index),
            ..
        }
        | ir::QueryExprKindIr::UserQuery {
            default_index: Some(default_index),
            ..
        } => {
            annotations.insert(
                "defaultIndex".to_owned(),
                ir::RequestValueIr::String(default_index.clone()),
            );
        }
        _ => {}
    }
    annotations
}

pub(super) fn query_expr_ir_vector<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::QueryExprIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::QueryExpr<'a>>>,
    >,
> {
    let values = values
        .iter()
        .map(|value| query_expr_ir_offset(builder, value))
        .collect::<Vec<_>>();
    Some(builder.create_vector(&values))
}

pub(super) fn query_children_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::QueryExprIr],
    kind: api::QueryExprKind,
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let clauses = query_expr_ir_vector(builder, values);
    let union = match kind {
        api::QueryExprKind::QueryPhrase => {
            api::QueryPhrase::create(builder, &api::QueryPhraseArgs { terms: clauses })
                .as_union_value()
        }
        api::QueryExprKind::QueryNear => {
            api::QueryNear::create(builder, &api::QueryNearArgs { terms: clauses }).as_union_value()
        }
        api::QueryExprKind::QueryONear => {
            api::QueryONear::create(builder, &api::QueryONearArgs { terms: clauses })
                .as_union_value()
        }
        api::QueryExprKind::QuerySameElement => {
            api::QuerySameElement::create(builder, &api::QuerySameElementArgs { clauses })
                .as_union_value()
        }
        api::QueryExprKind::QueryEquiv => {
            api::QueryEquiv::create(builder, &api::QueryEquivArgs { clauses }).as_union_value()
        }
        api::QueryExprKind::QueryWeakAnd => {
            api::QueryWeakAnd::create(builder, &api::QueryWeakAndArgs { clauses }).as_union_value()
        }
        _ => unreachable!(),
    };
    (kind, union)
}

pub(super) fn weighted_query_union<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &ir::QueryFieldRefIr,
    items: &[ir::WeightedQueryItemIr],
    kind: api::QueryExprKind,
) -> (
    api::QueryExprKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let field = query_field_ref_ir_offset(builder, field);
    let items = items
        .iter()
        .map(|item| {
            let key = request_value_offset(builder, &item.key);
            api::WeightedQueryItem::create(
                builder,
                &api::WeightedQueryItemArgs {
                    key: Some(key),
                    weight: item.weight,
                },
            )
        })
        .collect::<Vec<_>>();
    let items = Some(builder.create_vector(&items));
    let union = match kind {
        api::QueryExprKind::QueryDotProduct => api::QueryDotProduct::create(
            builder,
            &api::QueryDotProductArgs {
                field: Some(field),
                items,
            },
        )
        .as_union_value(),
        api::QueryExprKind::QueryWeightedSet => api::QueryWeightedSet::create(
            builder,
            &api::QueryWeightedSetArgs {
                field: Some(field),
                items,
            },
        )
        .as_union_value(),
        api::QueryExprKind::QueryWand => api::QueryWand::create(
            builder,
            &api::QueryWandArgs {
                field: Some(field),
                items,
            },
        )
        .as_union_value(),
        _ => unreachable!(),
    };
    (kind, union)
}

pub(super) fn query_field_ref_ir_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &ir::QueryFieldRefIr,
) -> flatbuffers::WIPOffset<api::QueryFieldRef<'a>> {
    let path = builder.create_string(field.path.as_str());
    let element_index = field.element_index.map(api::OptionalUInt32::new);
    api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(path),
            element_index: element_index.as_ref(),
        },
    )
}

pub(super) fn query_range_bound_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    bound: &ir::QueryRangeBoundIr,
) -> flatbuffers::WIPOffset<api::RangeBound<'a>> {
    let (bound_type, bound) = match bound {
        ir::QueryRangeBoundIr::Finite(value) => {
            let value = request_value_offset(builder, value);
            (
                api::RangeBoundKind::FiniteRangeBound,
                api::FiniteRangeBound::create(
                    builder,
                    &api::FiniteRangeBoundArgs { value: Some(value) },
                )
                .as_union_value(),
            )
        }
        ir::QueryRangeBoundIr::NegativeInfinity => (
            api::RangeBoundKind::NegativeInfinityBound,
            api::NegativeInfinityBound::create(builder, &api::NegativeInfinityBoundArgs {})
                .as_union_value(),
        ),
        ir::QueryRangeBoundIr::PositiveInfinity => (
            api::RangeBoundKind::PositiveInfinityBound,
            api::PositiveInfinityBound::create(builder, &api::PositiveInfinityBoundArgs {})
                .as_union_value(),
        ),
        ir::QueryRangeBoundIr::Unbounded => (
            api::RangeBoundKind::UnboundedRangeBound,
            api::UnboundedRangeBound::create(builder, &api::UnboundedRangeBoundArgs {})
                .as_union_value(),
        ),
    };
    api::RangeBound::create(
        builder,
        &api::RangeBoundArgs {
            bound_type,
            bound: Some(bound),
        },
    )
}
