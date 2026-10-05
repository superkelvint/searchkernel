use super::*;

pub fn inspect_query_result_to_api(request: &crate::model::SearchRequestIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = resolved_request_offset(&mut builder, request);
    let root = api::InspectQueryResult::create(
        &mut builder,
        &api::InspectQueryResultArgs {
            resolved_request: Some(request),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode an InspectQuery result into an existing transport builder.
pub fn inspect_query_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &crate::model::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::InspectQueryResult<'a>> {
    let resolved_request = resolved_request_offset(builder, request);
    api::InspectQueryResult::create(
        builder,
        &api::InspectQueryResultArgs {
            resolved_request: Some(resolved_request),
        },
    )
}

/// Encode only the normalized `SearchRequest` table.
pub fn search_request_ir_to_api(request: &crate::model::SearchRequestIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = search_request_offset(&mut builder, request);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub(super) fn search_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::SearchRequest<'a>> {
    let query = query_source_offset(builder, &request.query);

    let search_fields = match &request.query {
        ir::QueryIr::Text(_) => Some(search_field_selection_offset(
            builder,
            &request.search_fields,
        )),
        _ => None,
    };
    let filter = request.filter.as_ref().map(|value| {
        let expression = filter_offset(builder, value);
        let expressions = builder.create_vector(&[expression]);
        api::FilterRequest::create(
            builder,
            &api::FilterRequestArgs {
                structured: Some(expressions),
                sources: None,
                parameters: None,
                textual: None,
            },
        )
    });
    let sorts = request
        .sort
        .iter()
        .map(|value| sort_offset(builder, value))
        .collect::<Vec<_>>();
    let sort = builder.create_vector(&sorts);
    let ranking = ranking_request_offset(builder, &request.ranking_request);
    let limit = api::OptionalUInt32::new(request.limit);
    let offset = api::OptionalUInt32::new(request.offset);
    let presentation =
        presentation_offset(builder, request).expect("presentation is explicit in normalized IR");
    let timeout = request
        .timeout
        .map(|value| api::OptionalUInt64::new(value.as_millis() as u64));
    let inputs = query_inputs_offset(builder, &request.inputs);
    let query_model = query_model_offset(builder, &request.query_model);
    let properties = request_value_entries_offset(builder, &request.properties);
    let query_profile = request
        .query_profile
        .as_ref()
        .map(|value| builder.create_string(value));
    let search_chain = request
        .search_chain
        .as_ref()
        .map(|value| builder.create_string(value));
    let grouping = grouping_offsets(builder, &request.grouping);

    api::SearchRequest::create(
        builder,
        &api::SearchRequestArgs {
            diagnostic_source: request.diagnostic_source,
            query: Some(query),
            search_fields,
            filters: filter,
            sort: Some(sort),
            ranking: Some(ranking),
            limit: Some(&limit),
            offset: Some(&offset),
            timeout_ms: timeout.as_ref(),
            grouping,
            inputs,
            query_model,
            presentation: Some(presentation),
            properties,
            query_profile,
            search_chain,
        },
    )
}

pub(super) fn resolved_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::ResolvedSearchRequest<'a>> {
    let query = query_source_offset(builder, &request.query);
    let search_fields = search_fields_as_vector(builder, &request.search_fields);
    let filters = request.filter.as_ref().map(|value| {
        let expression = filter_offset(builder, value);
        builder.create_vector(&[expression])
    });
    let sorts = request
        .sort
        .iter()
        .map(|value| resolved_sort_offset(builder, value))
        .collect::<Vec<_>>();
    let sort = builder.create_vector(&sorts);
    let ranking = ranking_request_offset(builder, &request.ranking_request);
    let limit = api::OptionalUInt32::new(request.limit);
    let offset = api::OptionalUInt32::new(request.offset);
    let timeout = request
        .timeout
        .map(|value| api::OptionalUInt64::new(value.as_millis() as u64));
    let inputs = query_inputs_offset(builder, &request.inputs);
    let query_model = query_model_offset(builder, &request.query_model);
    let presentation = presentation_offset(builder, request);
    let properties = request_value_entries_offset(builder, &request.properties);
    let query_profile = request
        .query_profile
        .as_ref()
        .map(|value| builder.create_string(value));
    let search_chain = request
        .search_chain
        .as_ref()
        .map(|value| builder.create_string(value));
    let grouping = resolved_grouping_offsets(builder, &request.grouping);
    api::ResolvedSearchRequest::create(
        builder,
        &api::ResolvedSearchRequestArgs {
            diagnostic_source: request.diagnostic_source,
            query: Some(query),
            search_fields: Some(search_fields),
            filters,
            sort: Some(sort),
            grouping,
            ranking: Some(ranking),
            inputs,
            limit: limit.value(),
            offset: offset.value(),
            timeout_ms: timeout.as_ref(),
            query_model,
            presentation,
            properties,
            query_profile,
            search_chain,
        },
    )
}

pub(super) fn search_fields_as_vector<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::SearchFieldIr],
) -> flatbuffers::WIPOffset<
    flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::SearchField<'a>>>,
> {
    let offsets = fields
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
    builder.create_vector(&offsets)
}

pub(super) fn query_inputs_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    inputs: &std::collections::BTreeMap<String, ir::QueryInputIr>,
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::QueryInput<'a>>>,
    >,
> {
    (!inputs.is_empty()).then(|| {
        let values = inputs
            .iter()
            .map(|(name, input)| {
                let name = builder.create_string(name);
                let (input_type, input) = match input {
                    ir::QueryInputIr::Literal(value) => {
                        let value = request_value_offset(builder, value);
                        let value = api::LiteralQueryInput::create(
                            builder,
                            &api::LiteralQueryInputArgs { value: Some(value) },
                        );
                        (
                            api::QueryInputKind::LiteralQueryInput,
                            value.as_union_value(),
                        )
                    }
                    ir::QueryInputIr::Expression(value) => {
                        let value = builder.create_string(value);
                        let value = api::ExpressionQueryInput::create(
                            builder,
                            &api::ExpressionQueryInputArgs {
                                expression: Some(value),
                            },
                        );
                        (
                            api::QueryInputKind::ExpressionQueryInput,
                            value.as_union_value(),
                        )
                    }
                };
                api::QueryInput::create(
                    builder,
                    &api::QueryInputArgs {
                        name: Some(name),
                        input_type,
                        input: Some(input),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn query_model_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    model: &ir::QueryModelIr,
) -> Option<flatbuffers::WIPOffset<api::QueryModel<'a>>> {
    let populated = model.query_string.is_some()
        || model.default_index.is_some()
        || model.encoding.is_some()
        || model.filter.is_some()
        || model.language.is_some()
        || model.locale.is_some()
        || model.query_type.is_some()
        || !model.sources.is_empty()
        || !model.restrict.is_empty()
        || model.search_path.is_some()
        || model.search_group.is_some();
    populated.then(|| {
        let query_string = model
            .query_string
            .as_ref()
            .map(|value| builder.create_string(value));
        let default_index = model
            .default_index
            .as_ref()
            .map(|value| builder.create_string(value));
        let encoding = model
            .encoding
            .as_ref()
            .map(|value| builder.create_string(value));
        let filter = model
            .filter
            .as_ref()
            .map(|value| builder.create_string(value));
        let language = model
            .language
            .as_ref()
            .map(|value| builder.create_string(value.as_str()));
        let locale = model
            .locale
            .as_ref()
            .map(|value| builder.create_string(value));
        let source_offsets = model
            .sources
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        let sources = (!source_offsets.is_empty()).then(|| builder.create_vector(&source_offsets));
        let restrict_offsets = model
            .restrict
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        let restrict =
            (!restrict_offsets.is_empty()).then(|| builder.create_vector(&restrict_offsets));
        let search_path = model
            .search_path
            .as_ref()
            .map(|value| builder.create_string(value));
        let query_type = model.query_type.as_ref().map(|value| {
            let composite = value
                .composite
                .as_ref()
                .map(|value| builder.create_string(value));
            let tokenization = value
                .tokenization
                .as_ref()
                .map(|value| builder.create_string(value));
            let syntax = value
                .syntax
                .as_ref()
                .map(|value| builder.create_string(value));
            let profile = value
                .profile
                .as_ref()
                .map(|value| builder.create_string(value));
            let is_yql_default = value.is_yql_default.map(api::OptionalBool::new);
            api::QueryModelType::create(
                builder,
                &api::QueryModelTypeArgs {
                    composite,
                    tokenization,
                    syntax,
                    profile,
                    is_yql_default: is_yql_default.as_ref(),
                },
            )
        });
        let group = model.search_group.map(api::OptionalUInt32::new);
        api::QueryModel::create(
            builder,
            &api::QueryModelArgs {
                query_string,
                default_index,
                encoding,
                filter,
                language,
                locale,
                type_: query_type,
                sources,
                restrict,
                search_path,
                search_group: group.as_ref(),
            },
        )
    })
}

pub(super) fn presentation_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> Option<flatbuffers::WIPOffset<api::PresentationRequest<'a>>> {
    let projection = projection_offset(builder, &request.projection);
    let highlighting = match &request.highlighting {
        ir::HighlightingIr::Disabled => None,
        ir::HighlightingIr::Requested { fields, .. } => Some(highlighting_offset(builder, fields)),
    };
    let summary = request
        .presentation
        .summary
        .as_ref()
        .map(|value| builder.create_string(value));
    let format = request
        .presentation
        .format
        .as_ref()
        .map(|value| builder.create_string(value));
    let bolding = request.presentation.bolding.map(api::OptionalBool::new);
    let timing = request.presentation.timing.map(api::OptionalBool::new);
    Some(api::PresentationRequest::create(
        builder,
        &api::PresentationRequestArgs {
            summary,
            fields: Some(projection),
            highlighting,
            format,
            bolding: bolding.as_ref(),
            timing: timing.as_ref(),
        },
    ))
}
