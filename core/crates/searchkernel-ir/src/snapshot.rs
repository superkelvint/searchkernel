//! Deterministic internal JSON snapshots of owned portable IR.
//!
//! This is deliberately separate from the public JSON and transport codecs.
//! The serializers destructure owned IR structs and exhaustively match their
//! enums so adding an IR member or variant requires updating the snapshot
//! format at compile time.

use crate::model::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::time::Duration;

pub fn search_request(value: &SearchRequestIr) -> Value {
    let SearchRequestIr {
        query,
        search_fields,
        filter,
        sort,
        grouping,
        ranking_request,
        inputs,
        limit,
        offset,
        projection,
        highlighting,
        language,
        query_model,
        presentation,
        properties,
        query_profile,
        search_chain,
        timeout,
        prefix_policy,
        typo_policy,
    } = value;

    json!({
        "query": query_json(query),
        "search_fields": search_fields.iter().map(search_field_json).collect::<Vec<_>>(),
        "filter": filter.as_ref().map(filter_json),
        "sort": sort.iter().map(sort_json).collect::<Vec<_>>(),
        "grouping": grouping.iter().map(grouping_request_json).collect::<Vec<_>>(),
        "ranking_request": ranking_request_json(ranking_request),
        "inputs": inputs.iter().map(|(key, value)| (key.clone(), query_input_json(value))).collect::<Map<_, _>>(),
        "limit": limit,
        "offset": offset,
        "projection": projection_json(projection),
        "highlighting": highlighting_json(highlighting),
        "language": language.as_ref().map(LanguageHint::as_str),
        "query_model": query_model_json(query_model),
        "presentation": presentation_json(presentation),
        "properties": request_map_json(properties),
        "query_profile": query_profile,
        "search_chain": search_chain,
        "timeout": timeout_json(*timeout),
        "prefix_policy": prefix_policy_json(*prefix_policy),
        "typo_policy": typo_policy_json(*typo_policy),
    })
}

pub fn index_definition(value: &IndexDefinitionIr) -> Value {
    let IndexDefinitionIr {
        name,
        fields,
        default_search_fields,
        properties,
        document_type,
        resolved_document_type: _,
        schema_inherits,
        document_inherits,
        structs,
        fieldsets,
        schema_fields,
        imported_fields,
        document_summaries,
        rank_profiles,
        constants,
        onnx_models,
    } = value;

    json!({
        "name": name.as_ref().map(IndexName::as_str),
        "fields": fields.iter().map(field_json).collect::<Vec<_>>(),
        "default_search_fields": default_search_fields.iter().map(search_field_json).collect::<Vec<_>>(),
        "properties": request_map_json(properties),
        "document_type": document_type,
        "schema_inherits": schema_inherits,
        "document_inherits": document_inherits,
        "structs": structs.iter().map(named_struct_json).collect::<Vec<_>>(),
        "fieldsets": fieldsets.iter().map(fieldset_json).collect::<Vec<_>>(),
        "schema_fields": schema_fields.iter().map(field_json).collect::<Vec<_>>(),
        "imported_fields": imported_fields.iter().map(imported_field_json).collect::<Vec<_>>(),
        "document_summaries": document_summaries.iter().map(document_summary_json).collect::<Vec<_>>(),
        "rank_profiles": rank_profiles.iter().map(rank_profile_json).collect::<Vec<_>>(),
        "constants": constants.iter().map(constant_json).collect::<Vec<_>>(),
        "onnx_models": onnx_models.iter().map(onnx_model_json).collect::<Vec<_>>(),
    })
}

/// Snapshot a standalone field IR value using the same canonical representation
/// used by an index definition.
pub fn field(value: &FieldIr) -> Value {
    field_json(value)
}

/// Snapshot a standalone query IR value.
pub fn query(value: &QueryIr) -> Value {
    query_json(value)
}

/// Snapshot a standalone filter IR value.
pub fn filter(value: &FilterIr) -> Value {
    filter_json(value)
}

/// Snapshot a standalone sort IR value.
pub fn sort(value: &SortIr) -> Value {
    sort_json(value)
}

fn string_map_json(values: &BTreeMap<String, String>) -> Value {
    json!(values)
}

fn request_map_json(values: &BTreeMap<String, RequestValueIr>) -> Value {
    values
        .iter()
        .map(|(key, value)| (key.clone(), request_value_json(value)))
        .collect::<Map<_, _>>()
        .into()
}

fn timeout_json(value: Option<Duration>) -> Option<u128> {
    value.map(|value| value.as_millis())
}

fn prefix_policy_json(value: PrefixPolicy) -> &'static str {
    match value {
        PrefixPolicy::Disabled => "disabled",
    }
}

fn typo_policy_json(value: TypoPolicy) -> &'static str {
    match value {
        TypoPolicy::Disabled => "disabled",
    }
}

fn ranking_request_json(value: &RankingRequestIr) -> Value {
    let RankingRequestIr {
        profile,
        properties,
    } = value;
    json!({"profile": profile, "properties": properties})
}

fn primitive_type_json(value: PrimitiveDataTypeIr) -> &'static str {
    match value {
        PrimitiveDataTypeIr::String => "string",
        PrimitiveDataTypeIr::Bool => "bool",
        PrimitiveDataTypeIr::Byte => "byte",
        PrimitiveDataTypeIr::Int32 => "int32",
        PrimitiveDataTypeIr::Int64 => "int64",
        PrimitiveDataTypeIr::Float32 => "float32",
        PrimitiveDataTypeIr::Float64 => "float64",
        PrimitiveDataTypeIr::Raw => "raw",
    }
}

fn data_type_json(value: &DataTypeIr) -> Value {
    match value {
        DataTypeIr::Primitive(value) => {
            json!({"kind":"primitive","type":primitive_type_json(*value)})
        }
        DataTypeIr::Tensor { type_spec } => json!({"kind":"tensor","type_spec":type_spec}),
        DataTypeIr::Array(value) => json!({"kind":"array","element":data_type_json(value)}),
        DataTypeIr::Map {
            key_type,
            value_type,
        } => {
            json!({"kind":"map","key":data_type_json(key_type),"value":data_type_json(value_type)})
        }
        DataTypeIr::WeightedSet(value) => {
            json!({"kind":"weighted_set","element":data_type_json(value)})
        }
        DataTypeIr::Struct { fields } => {
            json!({"kind":"struct","fields":fields.iter().map(struct_field_json).collect::<Vec<_>>() })
        }
        DataTypeIr::StructReference { name } => json!({"kind":"struct_reference","name":name}),
        DataTypeIr::Predicate => json!({"kind":"predicate"}),
        DataTypeIr::Position => json!({"kind":"position"}),
        DataTypeIr::Reference { document_type } => {
            json!({"kind":"reference","document_type":document_type})
        }
    }
}

fn struct_field_json(value: &StructFieldIr) -> Value {
    let StructFieldIr { name, data_type } = value;
    json!({"name":name,"data_type":data_type_json(data_type)})
}

fn matching_mode_json(value: MatchingModeIr) -> &'static str {
    match value {
        MatchingModeIr::Unspecified => "unspecified",
        MatchingModeIr::Text => "text",
        MatchingModeIr::Word => "word",
        MatchingModeIr::Exact => "exact",
        MatchingModeIr::Gram => "gram",
    }
}

fn matching_json(value: &MatchingConfigIr) -> Value {
    let MatchingConfigIr { mode, options } = value;
    json!({"mode":matching_mode_json(*mode),"options":request_map_json(options)})
}

fn hnsw_json(value: &HnswIndexConfigIr) -> Value {
    let HnswIndexConfigIr {
        max_links_per_node,
        neighbors_to_explore_at_insert,
        options,
    } = value;
    json!({"max_links_per_node":max_links_per_node,"neighbors_to_explore_at_insert":neighbors_to_explore_at_insert,"options":request_map_json(options)})
}

fn index_config_json(value: &IndexConfigIr) -> Value {
    let IndexConfigIr {
        enabled,
        hnsw,
        options,
    } = value;
    json!({"enabled":enabled,"hnsw":hnsw.as_ref().map(hnsw_json),"options":request_map_json(options)})
}

fn distance_metric_json(value: DistanceMetricIr) -> &'static str {
    match value {
        DistanceMetricIr::Unspecified => "unspecified",
        DistanceMetricIr::Euclidean => "euclidean",
        DistanceMetricIr::Angular => "angular",
        DistanceMetricIr::DotProduct => "dot_product",
        DistanceMetricIr::PrenormalizedAngular => "prenormalized_angular",
        DistanceMetricIr::GeoDegrees => "geo_degrees",
        DistanceMetricIr::Hamming => "hamming",
    }
}

fn attribute_json(value: &AttributeConfigIr) -> Value {
    let AttributeConfigIr {
        enabled,
        fast_search,
        fast_access,
        distance_metric,
        options,
    } = value;
    json!({"enabled":enabled,"fast_search":fast_search,"fast_access":fast_access,"distance_metric":distance_metric_json(*distance_metric),"options":request_map_json(options)})
}

fn summary_config_json(value: &SummaryConfigIr) -> Value {
    let SummaryConfigIr { enabled, options } = value;
    json!({"enabled":enabled,"options":request_map_json(options)})
}

fn field_json(value: &FieldIr) -> Value {
    let FieldIr {
        name,
        data_type,
        analyzer,
        capabilities,
        matching,
        index,
        attribute,
        summary,
        indexing_script,
        properties,
        struct_fields,
    } = value;
    let FieldCapabilities {
        stored,
        searchable,
        filterable,
        sortable,
        groupable,
    } = capabilities;
    json!({
        "name": name.as_str(),
        "data_type": data_type_json(data_type),
        "analyzer": analyzer.as_ref().map(analyzer_json),
        "capabilities": {"stored":stored,"searchable":searchable,"filterable":filterable,"sortable":sortable,"groupable":groupable},
        "matching": matching.as_ref().map(matching_json),
        "index": index.as_ref().map(index_config_json),
        "attribute": attribute.as_ref().map(attribute_json),
        "summary": summary.as_ref().map(summary_config_json),
        "indexing_script": indexing_script,
        "properties": request_map_json(properties),
        "struct_fields": struct_fields.iter().map(struct_field_config_json).collect::<Vec<_>>(),
    })
}

fn analyzer_json(value: &AnalyzerIr) -> Value {
    let AnalyzerIr {
        name,
        version,
        options,
    } = value;
    json!({"name":name,"version":version,"options":request_map_json(options)})
}

fn struct_field_config_json(value: &StructFieldConfigIr) -> Value {
    let StructFieldConfigIr {
        path,
        indexing_script,
        matching,
        index,
        attribute,
        summary,
        rank,
        query_commands,
        properties,
    } = value;
    json!({"path":path,"indexing_script":indexing_script,"matching":matching.as_ref().map(matching_json),"index":index.as_ref().map(index_config_json),"attribute":attribute.as_ref().map(attribute_json),"summary":summary.as_ref().map(summary_config_json),"rank":rank,"query_commands":query_commands,"properties":request_map_json(properties)})
}

fn search_field_json(value: &SearchFieldIr) -> Value {
    let SearchFieldIr { field, weight } = value;
    json!({"field":field.as_str(),"weight":weight.get()})
}

fn scalar_json(value: &ScalarValueIr) -> Value {
    match value {
        ScalarValueIr::Bool(value) => json!({"bool":value}),
        ScalarValueIr::Int64(value) => json!({"int64":value}),
        ScalarValueIr::Float64(value) => json!({"float64":value}),
        ScalarValueIr::String(value) => json!({"string":value}),
    }
}

fn request_value_json(value: &RequestValueIr) -> Value {
    match value {
        RequestValueIr::Null => Value::Null,
        RequestValueIr::Bool(value) => json!({"bool":value}),
        RequestValueIr::Int64(value) => json!({"int64":value}),
        RequestValueIr::UInt64(value) => json!({"uint64":value}),
        RequestValueIr::Float64(value) => json!({"float64":value}),
        RequestValueIr::NonFiniteFloat(value) => json!({"non_finite":non_finite_json(*value)}),
        RequestValueIr::String(value) => json!({"string":value}),
        RequestValueIr::Bytes(value) => json!({"bytes":value}),
        RequestValueIr::List(value) => {
            json!({"list":value.iter().map(request_value_json).collect::<Vec<_>>() })
        }
        RequestValueIr::Object(value) => json!({"object":request_map_json(value)}),
        RequestValueIr::Tensor(value) => json!({"tensor":tensor_json(value)}),
    }
}

fn non_finite_json(value: RequestNonFiniteFloatIr) -> &'static str {
    match value {
        RequestNonFiniteFloatIr::NaN => "nan",
        RequestNonFiniteFloatIr::PositiveInfinity => "positive_infinity",
        RequestNonFiniteFloatIr::NegativeInfinity => "negative_infinity",
    }
}

fn tensor_json(value: &RequestTensorIr) -> Value {
    let RequestTensorIr { type_spec, payload } = value;
    let payload = match payload {
        RequestTensorPayloadIr::Dense(values) => json!({"dense":values}),
        RequestTensorPayloadIr::Sparse(values) => {
            json!({"sparse":values.iter().map(tensor_cell_json).collect::<Vec<_>>() })
        }
        RequestTensorPayloadIr::Mixed(values) => {
            json!({"mixed":values.iter().map(tensor_block_json).collect::<Vec<_>>() })
        }
    };
    json!({"type_spec":type_spec,"payload":payload})
}

fn tensor_cell_json(value: &RequestTensorCellIr) -> Value {
    let RequestTensorCellIr { address, value } = value;
    json!({"address":address,"value":value})
}

fn tensor_block_json(value: &RequestTensorBlockIr) -> Value {
    let RequestTensorBlockIr { address, values } = value;
    json!({"address":address,"values":values})
}

fn query_input_json(value: &QueryInputIr) -> Value {
    match value {
        QueryInputIr::Literal(value) => json!({"literal":request_value_json(value)}),
        QueryInputIr::Expression(value) => json!({"expression":value}),
    }
}

fn query_json(value: &QueryIr) -> Value {
    match value {
        QueryIr::MatchAll => json!({"kind":"match_all"}),
        QueryIr::Text(value) => json!({"kind":"text","value":value}),
        QueryIr::Yql { source, parameters } => {
            json!({"kind":"yql","source":source,"parameters":request_map_json(parameters)})
        }
        QueryIr::Typed(value) => json!({"kind":"typed","query":query_expr_json(value)}),
    }
}

fn query_field_json(value: &QueryFieldRefIr) -> Value {
    let QueryFieldRefIr {
        path,
        element_index,
    } = value;
    json!({"path":path.as_str(),"element_index":element_index})
}

fn query_range_bound_json(value: &QueryRangeBoundIr) -> Value {
    match value {
        QueryRangeBoundIr::Finite(value) => {
            json!({"kind":"finite","value":request_value_json(value)})
        }
        QueryRangeBoundIr::NegativeInfinity => json!({"kind":"negative_infinity"}),
        QueryRangeBoundIr::PositiveInfinity => json!({"kind":"positive_infinity"}),
        QueryRangeBoundIr::Unbounded => json!({"kind":"unbounded"}),
    }
}

fn query_expr_json(value: &QueryExprIr) -> Value {
    let QueryExprIr { kind, annotations } = value;
    let mut output = query_expr_kind_json(kind);
    output["annotations"] = request_map_json(annotations);
    output
}

fn query_expr_kind_json(value: &QueryExprKindIr) -> Value {
    match value {
        QueryExprKindIr::Term { value } => json!({"kind":"term","value":request_value_json(value)}),
        QueryExprKindIr::Contains { field, item } => {
            json!({"kind":"contains","field":query_field_json(field),"item":query_expr_json(item)})
        }
        QueryExprKindIr::And(values) => {
            json!({"kind":"and","children":values.iter().map(query_expr_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Or(values) => {
            json!({"kind":"or","children":values.iter().map(query_expr_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Not(value) => json!({"kind":"not","child":query_expr_json(value)}),
        QueryExprKindIr::True => json!({"kind":"true"}),
        QueryExprKindIr::False => json!({"kind":"false"}),
        QueryExprKindIr::Comparison { field, op, value } => {
            json!({"kind":"comparison","field":query_field_json(field),"op":comparison_op_json(*op),"value":request_value_json(value)})
        }
        QueryExprKindIr::Range {
            field,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            json!({"kind":"range","field":query_field_json(field),"lower":query_range_bound_json(lower),"upper":query_range_bound_json(upper),"include_lower":include_lower,"include_upper":include_upper})
        }
        QueryExprKindIr::In { field, values } => {
            json!({"kind":"in","field":query_field_json(field),"values":values.iter().map(request_value_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Phrase(values) => {
            json!({"kind":"phrase","children":values.iter().map(query_expr_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Near { children, distance } => {
            json!({"kind":"near","children":children.iter().map(query_expr_json).collect::<Vec<_>>(),"distance":distance})
        }
        QueryExprKindIr::ONear { children, distance } => {
            json!({"kind":"onear","children":children.iter().map(query_expr_json).collect::<Vec<_>>(),"distance":distance})
        }
        QueryExprKindIr::SameElement(values) => {
            json!({"kind":"same_element","children":values.iter().map(query_expr_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Equiv(values) => {
            json!({"kind":"equiv","children":values.iter().map(query_expr_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Alternatives(values) => {
            json!({"kind":"alternatives","values":values.iter().map(alternative_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Fuzzy {
            value,
            max_edit_distance,
            prefix_length,
            prefix,
        } => {
            json!({"kind":"fuzzy","value":request_value_json(value),"max_edit_distance":max_edit_distance,"prefix_length":prefix_length,"prefix":prefix})
        }
        QueryExprKindIr::Matches { field, pattern } => {
            json!({"kind":"matches","field":query_field_json(field),"pattern":pattern})
        }
        QueryExprKindIr::Text { text } => json!({"kind":"text","text":text}),
        QueryExprKindIr::UserInput {
            value,
            default_index,
            target_hits,
        } => {
            json!({"kind":"user_input","value":value,"default_index":default_index,"target_hits":target_hits})
        }
        QueryExprKindIr::UserQuery {
            default_index,
            target_hits,
        } => json!({"kind":"user_query","default_index":default_index,"target_hits":target_hits}),
        QueryExprKindIr::DotProduct { field, items } => {
            json!({"kind":"dot_product","field":query_field_json(field),"items":items.iter().map(weighted_item_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::WeightedSet { field, items } => {
            json!({"kind":"weighted_set","field":query_field_json(field),"items":items.iter().map(weighted_item_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Wand { field, items } => {
            json!({"kind":"wand","field":query_field_json(field),"items":items.iter().map(weighted_item_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::WeakAnd {
            children,
            target_hits,
        } => {
            json!({"kind":"weak_and","children":children.iter().map(query_expr_json).collect::<Vec<_>>(),"target_hits":target_hits})
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            json!({"kind":"rank","matching":query_expr_json(matching),"ranking":ranking.iter().map(query_expr_json).collect::<Vec<_>>() })
        }
        QueryExprKindIr::Labeled {
            query,
            label,
            score,
        } => json!({"kind":"labeled","query":query_expr_json(query),"label":label,"score":score}),
        QueryExprKindIr::NonEmpty(value) => {
            json!({"kind":"non_empty","query":query_expr_json(value)})
        }
        QueryExprKindIr::GeoLocation {
            field,
            latitude,
            longitude,
            radius,
        } => {
            json!({"kind":"geo_location","field":query_field_json(field),"latitude":latitude,"longitude":longitude,"radius":radius})
        }
        QueryExprKindIr::GeoBoundingBox {
            field,
            south,
            west,
            north,
            east,
        } => {
            json!({"kind":"geo_bounding_box","field":query_field_json(field),"south":south,"west":west,"north":north,"east":east})
        }
        QueryExprKindIr::NearestNeighbor {
            document_field,
            query_tensor,
        } => {
            json!({"kind":"nearest_neighbor","document_field":query_field_json(document_field),"query_tensor":query_tensor})
        }
        QueryExprKindIr::Predicate {
            field,
            features,
            range_features,
        } => {
            json!({"kind":"predicate","field":query_field_json(field),"features":request_map_json(features),"range_features":request_map_json(range_features)})
        }
    }
}

fn alternative_json(value: &QueryAlternativeIr) -> Value {
    let QueryAlternativeIr { query, exactness } = value;
    json!({"query":query_expr_json(query),"exactness":exactness})
}

fn weighted_item_json(value: &WeightedQueryItemIr) -> Value {
    let WeightedQueryItemIr { key, weight } = value;
    json!({"key":request_value_json(key),"weight":weight})
}

fn comparison_op_json(value: ComparisonOp) -> &'static str {
    match value {
        ComparisonOp::Eq => "eq",
        ComparisonOp::Ne => "ne",
        ComparisonOp::Lt => "lt",
        ComparisonOp::Lte => "lte",
        ComparisonOp::Gt => "gt",
        ComparisonOp::Gte => "gte",
    }
}

fn filter_json(value: &FilterIr) -> Value {
    match value {
        FilterIr::And(values) => {
            json!({"kind":"and","children":values.iter().map(filter_json).collect::<Vec<_>>() })
        }
        FilterIr::Or(values) => {
            json!({"kind":"or","children":values.iter().map(filter_json).collect::<Vec<_>>() })
        }
        FilterIr::Not(value) => json!({"kind":"not","child":filter_json(value)}),
        FilterIr::Comparison(value) => {
            json!({"kind":"comparison","field":value.field.as_str(),"op":comparison_op_json(value.op),"value":scalar_json(&value.value)})
        }
        FilterIr::Range(value) => {
            let RangeIr {
                field,
                lower,
                upper,
                include_lower,
                include_upper,
            } = value;
            json!({"kind":"range","field":field.as_str(),"lower":filter_bound_json(lower),"upper":filter_bound_json(upper),"include_lower":include_lower,"include_upper":include_upper})
        }
        FilterIr::In { field, values } => {
            json!({"kind":"in","field":field.as_str(),"values":values.iter().map(scalar_json).collect::<Vec<_>>() })
        }
        FilterIr::Exists { field } => json!({"kind":"exists","field":field.as_str()}),
    }
}

fn filter_bound_json(value: &FilterRangeBoundIr) -> Value {
    match value {
        FilterRangeBoundIr::Finite(value) => json!({"kind":"finite","value":scalar_json(value)}),
        FilterRangeBoundIr::NegativeInfinity => json!({"kind":"negative_infinity"}),
        FilterRangeBoundIr::PositiveInfinity => json!({"kind":"positive_infinity"}),
        FilterRangeBoundIr::Unbounded => json!({"kind":"unbounded"}),
    }
}

fn sort_json(value: &SortIr) -> Value {
    let SortIr {
        target,
        direction,
        missing,
        annotations,
    } = value;
    let target = match target {
        SortTargetIr::Field(field) => json!({"kind":"field","field":field.as_str()}),
        SortTargetIr::Relevance => json!({"kind":"relevance"}),
        SortTargetIr::RankFeature(feature) => json!({"kind":"rank_feature","feature":feature}),
    };
    let missing = match missing {
        MissingPlacement::First => json!({"kind":"first"}),
        MissingPlacement::Last => json!({"kind":"last"}),
        MissingPlacement::As(value) => json!({"kind":"as","value":request_value_json(value)}),
    };
    json!({"target":target,"direction":match direction { SortDirection::Asc => "asc", SortDirection::Desc => "desc" },"missing":missing,"annotations":request_map_json(annotations)})
}

fn projection_json(value: &ProjectionIr) -> Value {
    let ProjectionIr { fields } = value;
    json!(fields.iter().map(FieldName::as_str).collect::<Vec<_>>())
}

fn highlighting_json(value: &HighlightingIr) -> Value {
    match value {
        HighlightingIr::Disabled => json!({"kind":"disabled"}),
        HighlightingIr::Requested { fields, .. } => {
            json!({"kind":"requested","fields":fields.iter().map(FieldName::as_str).collect::<Vec<_>>() })
        }
    }
}

fn query_model_json(value: &QueryModelIr) -> Value {
    let QueryModelIr {
        query_string,
        default_index,
        encoding,
        filter,
        language,
        locale,
        query_type,
        sources,
        restrict,
        search_path,
        search_group,
    } = value;
    json!({"query_string":query_string,"default_index":default_index,"encoding":encoding,"filter":filter,"language":language.as_ref().map(LanguageHint::as_str),"locale":locale,"query_type":query_type.as_ref().map(query_model_type_json),"sources":sources,"restrict":restrict,"search_path":search_path,"search_group":search_group})
}

fn query_model_type_json(value: &QueryModelTypeIr) -> Value {
    let QueryModelTypeIr {
        composite,
        tokenization,
        syntax,
        profile,
        is_yql_default,
    } = value;
    json!({"composite":composite,"tokenization":tokenization,"syntax":syntax,"profile":profile,"is_yql_default":is_yql_default})
}

fn presentation_json(value: &PresentationIr) -> Value {
    let PresentationIr {
        summary,
        format,
        bolding,
        timing,
    } = value;
    json!({"summary":summary,"format":format,"bolding":bolding,"timing":timing})
}

fn grouping_request_json(value: &GroupingRequestIr) -> Value {
    let GroupingRequestIr {
        root,
        continuations,
    } = value;
    json!({"root":grouping_all_json(root),"continuations":continuations})
}

fn grouping_all_json(value: &GroupingAllIr) -> Value {
    let GroupingAllIr { operations, label } = value;
    json!({"operations":operations.iter().map(grouping_operation_json).collect::<Vec<_>>(),"label":label})
}

fn grouping_operation_json(value: &GroupingOperationIr) -> Value {
    match value {
        GroupingOperationIr::All(value) => json!({"kind":"all","value":grouping_all_json(value)}),
        GroupingOperationIr::Each(value) => json!({"kind":"each","value":grouping_all_json(value)}),
        GroupingOperationIr::Group(value) => {
            json!({"kind":"group","expression":grouping_expression_json(value)})
        }
        GroupingOperationIr::Output(values) => {
            json!({"kind":"output","values":values.iter().map(grouping_output_json).collect::<Vec<_>>() })
        }
        GroupingOperationIr::Max(value) => json!({"kind":"max","value":value}),
        GroupingOperationIr::Precision(value) => json!({"kind":"precision","value":value}),
        GroupingOperationIr::Order(values) => {
            json!({"kind":"order","expressions":values.iter().map(grouping_expression_json).collect::<Vec<_>>() })
        }
        GroupingOperationIr::Alias { name, expression } => {
            json!({"kind":"alias","name":name,"expression":grouping_expression_json(expression)})
        }
        GroupingOperationIr::Filter(value) => {
            json!({"kind":"filter","value":grouping_filter_json(value)})
        }
        GroupingOperationIr::Keep(value) => {
            json!({"kind":"keep","value":grouping_filter_json(value)})
        }
        GroupingOperationIr::Where(value) => {
            json!({"kind":"where","expression":grouping_expression_json(value)})
        }
    }
}

fn grouping_output_json(value: &GroupingOutputIr) -> Value {
    let GroupingOutputIr { label, expression } = value;
    json!({"label":label,"expression":grouping_expression_json(expression)})
}

fn grouping_expression_json(value: &GroupingExpressionIr) -> Value {
    match value {
        GroupingExpressionIr::Attribute(value) => json!({"kind":"attribute","value":value}),
        GroupingExpressionIr::Literal(value) => {
            json!({"kind":"literal","value":request_value_json(value)})
        }
        GroupingExpressionIr::Function { name, arguments } => {
            json!({"kind":"function","name":name,"arguments":arguments.iter().map(grouping_expression_json).collect::<Vec<_>>() })
        }
        GroupingExpressionIr::Positive(value) => {
            json!({"kind":"positive","value":grouping_expression_json(value)})
        }
        GroupingExpressionIr::Negative(value) => {
            json!({"kind":"negative","value":grouping_expression_json(value)})
        }
        GroupingExpressionIr::Binary {
            operator,
            left,
            right,
        } => {
            json!({"kind":"binary","operator":grouping_binary_operator_json(*operator),"left":grouping_expression_json(left),"right":grouping_expression_json(right)})
        }
        GroupingExpressionIr::Alias(value) => json!({"kind":"alias","value":value}),
        GroupingExpressionIr::Member { base, member } => {
            json!({"kind":"member","base":grouping_expression_json(base),"member":member})
        }
        GroupingExpressionIr::MapLookup { base, key } => {
            json!({"kind":"map_lookup","base":grouping_expression_json(base),"key":grouping_expression_json(key)})
        }
        GroupingExpressionIr::Bucket(value) => {
            json!({"kind":"bucket","value":grouping_bucket_json(value)})
        }
        GroupingExpressionIr::Aggregation(value) => {
            json!({"kind":"aggregation","value":grouping_aggregation_json(value)})
        }
    }
}

fn grouping_binary_operator_json(value: GroupingBinaryOperatorIr) -> &'static str {
    match value {
        GroupingBinaryOperatorIr::Add => "add",
        GroupingBinaryOperatorIr::Subtract => "subtract",
        GroupingBinaryOperatorIr::Multiply => "multiply",
        GroupingBinaryOperatorIr::Divide => "divide",
        GroupingBinaryOperatorIr::Modulo => "modulo",
    }
}

fn grouping_bucket_json(value: &GroupingBucketIr) -> Value {
    let GroupingBucketIr {
        lower,
        upper,
        include_lower,
        include_upper,
    } = value;
    json!({"lower":grouping_bound_json(lower),"upper":grouping_bound_json(upper),"include_lower":include_lower,"include_upper":include_upper})
}

fn grouping_bound_json(value: &GroupingRangeBoundIr) -> Value {
    match value {
        GroupingRangeBoundIr::Finite(value) => {
            json!({"kind":"finite","value":request_value_json(value)})
        }
        GroupingRangeBoundIr::NegativeInfinity => json!({"kind":"negative_infinity"}),
        GroupingRangeBoundIr::PositiveInfinity => json!({"kind":"positive_infinity"}),
        GroupingRangeBoundIr::Unbounded => json!({"kind":"unbounded"}),
    }
}

fn grouping_aggregation_json(value: &GroupingAggregationIr) -> Value {
    match value {
        GroupingAggregationIr::Count => json!({"kind":"count"}),
        GroupingAggregationIr::Sum(value) => {
            json!({"kind":"sum","expression":grouping_expression_json(value)})
        }
        GroupingAggregationIr::Avg(value) => {
            json!({"kind":"avg","expression":grouping_expression_json(value)})
        }
        GroupingAggregationIr::Min(value) => {
            json!({"kind":"min","expression":grouping_expression_json(value)})
        }
        GroupingAggregationIr::Max(value) => {
            json!({"kind":"max","expression":grouping_expression_json(value)})
        }
        GroupingAggregationIr::Xor(value) => {
            json!({"kind":"xor","expression":grouping_expression_json(value)})
        }
        GroupingAggregationIr::StdDev(value) => {
            json!({"kind":"stddev","expression":grouping_expression_json(value)})
        }
        GroupingAggregationIr::Quantiles {
            quantiles,
            expression,
        } => {
            json!({"kind":"quantiles","quantiles":quantiles,"expression":grouping_expression_json(expression)})
        }
        GroupingAggregationIr::Summary(value) => json!({"kind":"summary","summary":value}),
    }
}

fn grouping_filter_json(value: &GroupingFilterIr) -> Value {
    match value {
        GroupingFilterIr::And(values) => {
            json!({"kind":"and","children":values.iter().map(grouping_filter_json).collect::<Vec<_>>() })
        }
        GroupingFilterIr::Or(values) => {
            json!({"kind":"or","children":values.iter().map(grouping_filter_json).collect::<Vec<_>>() })
        }
        GroupingFilterIr::Not(value) => json!({"kind":"not","child":grouping_filter_json(value)}),
        GroupingFilterIr::Regex {
            expression,
            pattern,
        } => {
            json!({"kind":"regex","expression":grouping_expression_json(expression),"pattern":pattern})
        }
        GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            json!({"kind":"range","expression":grouping_expression_json(expression),"lower":grouping_expression_json(lower),"upper":grouping_expression_json(upper),"include_lower":include_lower,"include_upper":include_upper})
        }
        GroupingFilterIr::IsTrue(value) => {
            json!({"kind":"is_true","expression":grouping_expression_json(value)})
        }
    }
}

fn named_struct_json(value: &NamedStructDefinitionIr) -> Value {
    let NamedStructDefinitionIr {
        name,
        fields,
        properties,
    } = value;
    json!({"name":name,"fields":fields.iter().map(struct_field_json).collect::<Vec<_>>(),"properties":request_map_json(properties)})
}

fn fieldset_json(value: &FieldSetDefinitionIr) -> Value {
    let FieldSetDefinitionIr {
        name,
        fields,
        matching,
        query_commands,
        properties,
    } = value;
    json!({"name":name,"fields":fields.iter().map(FieldName::as_str).collect::<Vec<_>>(),"matching":matching.as_ref().map(matching_json),"query_commands":query_commands,"properties":request_map_json(properties)})
}

fn imported_field_json(value: &ImportedFieldDefinitionIr) -> Value {
    let ImportedFieldDefinitionIr {
        reference_field,
        target_field,
        alias,
        properties,
    } = value;
    json!({"reference_field":reference_field.as_str(),"target_field":target_field,"alias":alias.as_str(),"properties":request_map_json(properties)})
}

fn document_summary_field_json(value: &DocumentSummaryFieldDefinitionIr) -> Value {
    let DocumentSummaryFieldDefinitionIr {
        name,
        source,
        data_type,
        struct_fields,
        properties,
    } = value;
    json!({"name":name,"source":source,"data_type":data_type_json(data_type),"struct_fields":struct_fields,"properties":request_map_json(properties)})
}

fn document_summary_json(value: &DocumentSummaryDefinitionIr) -> Value {
    let DocumentSummaryDefinitionIr {
        name,
        inherits,
        fields,
        from_disk,
        omit_summary_features,
        properties,
    } = value;
    json!({"name":name,"inherits":inherits,"fields":fields.iter().map(document_summary_field_json).collect::<Vec<_>>(),"from_disk":from_disk,"omit_summary_features":omit_summary_features,"properties":request_map_json(properties)})
}

fn rank_profile_input_json(value: &RankProfileInputIr) -> Value {
    let RankProfileInputIr {
        name,
        data_type,
        default_value,
    } = value;
    json!({"name":name,"data_type":data_type_json(data_type),"default_value":request_value_json(default_value)})
}

fn rank_function_json(value: &RankFunctionDefinitionIr) -> Value {
    let RankFunctionDefinitionIr {
        name,
        arguments,
        expression,
        inline,
    } = value;
    json!({"name":name,"arguments":arguments,"expression":expression,"inline":inline})
}

fn rank_phase_json(value: &RankPhaseDefinitionIr) -> Value {
    let RankPhaseDefinitionIr {
        expression,
        rerank_count,
        rank_score_drop_limit,
        properties,
        total_rerank_count,
        keep_rank_count,
        total_keep_rank_count,
    } = value;
    json!({"expression":expression,"rerank_count":rerank_count,"rank_score_drop_limit":rank_score_drop_limit,"properties":request_map_json(properties),"total_rerank_count":total_rerank_count,"keep_rank_count":keep_rank_count,"total_keep_rank_count":total_keep_rank_count})
}

fn constant_source_json(value: &ConstantSourceIr) -> Value {
    match value {
        ConstantSourceIr::Literal(value) => {
            json!({"kind":"literal","value":request_value_json(value)})
        }
        ConstantSourceIr::File { path } => json!({"kind":"file","path":path}),
    }
}

fn constant_json(value: &ConstantDefinitionIr) -> Value {
    let ConstantDefinitionIr {
        name,
        data_type,
        source,
        properties,
    } = value;
    json!({"name":name,"data_type":data_type_json(data_type),"source":constant_source_json(source),"properties":request_map_json(properties)})
}

fn onnx_model_json(value: &OnnxModelDefinitionIr) -> Value {
    let OnnxModelDefinitionIr {
        name,
        file,
        inputs,
        outputs,
        gpu_device,
        intraop_threads,
        interop_threads,
        properties,
    } = value;
    json!({"name":name,"file":file,"inputs":string_map_json(inputs),"outputs":string_map_json(outputs),"gpu_device":gpu_device,"intraop_threads":intraop_threads,"interop_threads":interop_threads,"properties":request_map_json(properties)})
}

fn rank_profile_json(value: &RankProfileDefinitionIr) -> Value {
    let RankProfileDefinitionIr {
        name,
        inherits,
        inputs,
        functions,
        first_phase,
        second_phase,
        global_phase,
        rank_features,
        match_features,
        summary_features,
        sort_features,
        constants,
        onnx_models,
        rank_properties,
        strict,
        properties,
    } = value;
    json!({"name":name,"inherits":inherits,"inputs":inputs.iter().map(rank_profile_input_json).collect::<Vec<_>>(),"functions":functions.iter().map(rank_function_json).collect::<Vec<_>>(),"first_phase":first_phase.as_ref().map(rank_phase_json),"second_phase":second_phase.as_ref().map(rank_phase_json),"global_phase":global_phase.as_ref().map(rank_phase_json),"rank_features":rank_features,"match_features":match_features,"summary_features":summary_features,"sort_features":sort_features,"constants":constants.iter().map(constant_json).collect::<Vec<_>>(),"onnx_models":onnx_models.iter().map(onnx_model_json).collect::<Vec<_>>(),"rank_properties":rank_properties,"strict":strict,"properties":request_map_json(properties)})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> SearchRequestIr {
        let field = FieldName::parse("title").unwrap();
        SearchRequestIr::new(
            QueryIr::MatchAll,
            vec![SearchFieldIr::new(
                field,
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            None,
            Vec::new(),
            10,
            0,
            ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
    }

    #[test]
    fn request_snapshot_observes_v2_control_state() {
        let base = search_request(&base_request());
        let mut ranking_properties = BTreeMap::new();
        ranking_properties.insert("model.version".to_owned(), "2".to_owned());
        let mut inputs = BTreeMap::new();
        inputs.insert(
            "ranking.features.query(foo)".to_owned(),
            QueryInputIr::Expression("input(foo)".to_owned()),
        );
        let changed = search_request(
            &base_request()
                .with_ranking_request("product_search", ranking_properties)
                .with_inputs(inputs)
                .with_timeout(Some(Duration::from_millis(250))),
        );

        assert_ne!(base, changed);
        assert_eq!(base.get("grouping").unwrap(), &json!([]));
        assert!(base.get("facets").is_none());
        assert!(base.get("ranking").is_none());
        assert_eq!(changed["ranking_request"]["profile"], "product_search");
        assert_eq!(changed["timeout"], 250);
    }

    #[test]
    fn index_snapshot_observes_schema_state() {
        let field = FieldIr::new(
            FieldName::parse("title").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        );
        let mut index = IndexDefinitionIr::new(
            Some(IndexName::parse("products").unwrap()),
            vec![field.clone()],
            Vec::new(),
        )
        .unwrap();
        let base = index_definition(&index);
        index.set_document_type("product");
        index.set_schema_fields(vec![field]);
        let changed = index_definition(&index);

        assert_ne!(base, changed);
        assert_eq!(changed["document_type"], "product");
        assert_eq!(changed["schema_fields"].as_array().unwrap().len(), 1);
    }
}
