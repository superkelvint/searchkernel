use super::*;
use searchkernel_protocol::protocol::v1::PROTOCOL_VERSION;

/// A component corpus case embedded in a valid transport envelope for JSON
/// round-trip testing. Direct component tables are not themselves transport
/// roots, so this preserves their wire shape inside the closest protocol root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportCase {
    Request(Vec<u8>),
    Response(Vec<u8>),
}
/// Build a component conformance case inside a canonical SKRQ or SKRS root.
/// This is test infrastructure; production JSON conversion remains BFBS-only.
pub fn build_component_transport_case(
    operation: &str,
    input: &JsonValue,
) -> StdResult<TransportCase, String> {
    let mut builder = WireBuilder::new();
    let request = |builder: &mut WireBuilder,
                   payload_type: transport::RequestPayload,
                   payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>| {
        let envelope = transport::RequestEnvelope::create(
            &mut builder.builder,
            &transport::RequestEnvelopeArgs {
                protocol_version: PROTOCOL_VERSION,
                target: None,
                payload_type,
                payload: Some(payload),
            },
        );
        builder
            .finish_with_identifier(envelope, Some("SKRQ"))
            .map(TransportCase::Request)
    };
    let response =
        |builder: &mut WireBuilder,
         payload_type: transport::ResponsePayload,
         payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>| {
            let envelope = transport::ResponseEnvelope::create(
                &mut builder.builder,
                &transport::ResponseEnvelopeArgs {
                    protocol_version: PROTOCOL_VERSION,
                    payload_type,
                    payload: Some(payload),
                },
            );
            builder
                .finish_with_identifier(envelope, Some("SKRS"))
                .map(TransportCase::Response)
        };

    match operation {
        "index_definition_to_ir" => {
            let index = builder.index(input)?;
            let payload = transport::DescribeResult::create(
                &mut builder.builder,
                &transport::DescribeResultArgs { index: Some(index) },
            );
            response(
                &mut builder,
                transport::ResponsePayload::SearchKernel_Wire_DescribeResult,
                payload.as_union_value(),
            )
        }
        "field_to_ir" => {
            let field = input.get("field").unwrap_or(input);
            let field = builder.field(field)?;
            let fields = builder.builder.create_vector(&[field]);
            let index = api::IndexDefinition::create(
                &mut builder.builder,
                &api::IndexDefinitionArgs {
                    fields: Some(fields),
                    ..Default::default()
                },
            );
            let payload = transport::DescribeResult::create(
                &mut builder.builder,
                &transport::DescribeResultArgs { index: Some(index) },
            );
            response(
                &mut builder,
                transport::ResponsePayload::SearchKernel_Wire_DescribeResult,
                payload.as_union_value(),
            )
        }
        "analyzer_to_ir" => {
            let payload = builder.analyze(input)?;
            request(
                &mut builder,
                transport::RequestPayload::SearchKernel_Wire_AnalyzeRequest,
                payload.as_union_value(),
            )
        }
        "document_to_ir" => {
            let document = builder.document(input.get("document").unwrap_or(input))?;
            let payload = api::InsertRequest::create(
                &mut builder.builder,
                &api::InsertRequestArgs {
                    document: Some(document),
                },
            );
            request(
                &mut builder,
                transport::RequestPayload::SearchKernel_Wire_InsertRequest,
                payload.as_union_value(),
            )
        }
        "value_to_ir" => {
            let field = input
                .get("field")
                .and_then(JsonValue::as_str)
                .unwrap_or("value");
            let value = input.get("value").cloned().unwrap_or_else(|| json!({}));
            let mut fields = Map::new();
            fields.insert(field.to_owned(), value);
            let document = builder.document(&json!({
                "id": "component-value",
                "fields": fields,
            }))?;
            let payload = api::InsertRequest::create(
                &mut builder.builder,
                &api::InsertRequestArgs {
                    document: Some(document),
                },
            );
            request(
                &mut builder,
                transport::RequestPayload::SearchKernel_Wire_InsertRequest,
                payload.as_union_value(),
            )
        }
        "search_fields_to_ir"
        | "projection_to_ir"
        | "highlighting_to_ir"
        | "comparison_to_ir"
        | "range_to_ir"
        | "filter_to_ir"
        | "sort_to_ir"
        | "facet_to_ir"
        | "ranking_to_ir"
        | "query_to_ir" => {
            let payload = builder.search(&component_search_input(operation, input))?;
            request(
                &mut builder,
                transport::RequestPayload::SearchKernel_Wire_SearchRequest,
                payload.as_union_value(),
            )
        }
        other => Err(format!("no component transport embedding for {other}")),
    }
}
fn component_search_input(operation: &str, input: &JsonValue) -> JsonValue {
    let mut object = input.as_object().cloned().unwrap_or_default();
    match operation {
        "search_fields_to_ir" => {
            object.remove("selection");
            if let Some(selection) = input.get("selection").filter(|value| !value.is_null()) {
                object.insert("search_fields".to_owned(), selection.clone());
            }
        }
        "projection_to_ir" => {
            object.remove("projection");
            if let Some(projection) = input.get("projection").filter(|value| !value.is_null()) {
                object.insert("projection".to_owned(), projection.clone());
            }
        }
        "highlighting_to_ir" => {
            object.remove("highlighting");
            if let Some(highlighting) = input.get("highlighting").filter(|value| !value.is_null()) {
                object.insert("highlighting".to_owned(), highlighting.clone());
            }
        }
        "comparison_to_ir" => {
            object.clear();
            object.insert(
                "filter".to_owned(),
                json!({"comparison": input.get("comparison").unwrap_or(input)}),
            );
        }
        "range_to_ir" => {
            object.clear();
            object.insert(
                "filter".to_owned(),
                json!({"range": input.get("range").unwrap_or(input)}),
            );
        }
        "filter_to_ir" => {
            object.clear();
            object.insert("filter".to_owned(), input.clone());
        }
        "sort_to_ir" => {
            object.clear();
            object.insert("sort".to_owned(), json!([input]));
        }
        "facet_to_ir" => {
            object.clear();
            object.insert("facets".to_owned(), json!([input]));
        }
        "ranking_to_ir" => {
            object.clear();
            object.insert("ranking".to_owned(), input.clone());
        }
        "query_to_ir" => {
            object.clear();
            if let Some(query) = input.get("query") {
                object.insert("query".to_owned(), query.clone());
            }
            if let Some(query_expr) = input.get("query_expr") {
                object.insert("query_expr".to_owned(), query_expr.clone());
            }
        }
        _ => {}
    }
    if operation != "query_to_ir"
        && !object.contains_key("query")
        && !object.contains_key("query_expr")
    {
        // Component cases need a valid enclosing request. Use the explicit
        // MatchAll source; an empty text source is intentionally invalid.
        object.insert("match_all".to_owned(), JsonValue::Bool(true));
    }
    JsonValue::Object(object)
}
