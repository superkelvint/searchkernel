use super::*;
mod filter;
mod schema;
mod search;
mod update;
mod value;

pub(super) struct WireBuilder {
    pub(super) builder: flatbuffers::FlatBufferBuilder<'static>,
}

impl WireBuilder {
    pub(in crate::runner) fn new() -> Self {
        Self {
            builder: flatbuffers::FlatBufferBuilder::new(),
        }
    }

    pub(in crate::runner) fn text(&mut self, value: &str) -> flatbuffers::WIPOffset<&'static str> {
        self.builder.create_string(value)
    }

    pub(in crate::runner) fn query_field(
        &mut self,
        value: &str,
    ) -> flatbuffers::WIPOffset<api::QueryFieldRef<'static>> {
        let path = self.text(value);
        api::QueryFieldRef::create(
            &mut self.builder,
            &api::QueryFieldRefArgs {
                path: Some(path),
                element_index: None,
            },
        )
    }

    pub(in crate::runner) fn finish<T: flatbuffers::Follow<'static>>(
        &mut self,
        root: flatbuffers::WIPOffset<T>,
    ) -> StdResult<Vec<u8>, String> {
        self.finish_with_identifier(root, None)
    }

    pub(in crate::runner) fn finish_with_identifier<T: flatbuffers::Follow<'static>>(
        &mut self,
        root: flatbuffers::WIPOffset<T>,
        identifier: Option<&str>,
    ) -> StdResult<Vec<u8>, String> {
        self.builder.finish(root, identifier);
        Ok(self.builder.finished_data().to_vec())
    }

    pub(in crate::runner) fn build(
        mut self,
        operation: &str,
        input: &JsonValue,
    ) -> StdResult<Vec<u8>, String> {
        match operation {
            "index_definition_to_ir" => {
                let root = self.index(input)?;
                self.finish(root)
            }
            "field_to_ir" => {
                let root = self.field(input.get("field").unwrap_or(input))?;
                self.finish(root)
            }
            "analyzer_to_ir" => {
                let root = self.analyzer(input.get("analyzer"))?;
                self.finish(root)
            }
            "value_to_ir" => {
                let root = self.value(input.get("value").unwrap_or(input))?;
                self.finish(root)
            }
            "document_to_ir" => {
                let root = self.document(input.get("document").unwrap_or(input))?;
                self.finish(root)
            }
            "search_fields_to_ir" => {
                let root = self.selection(input.get("selection").unwrap_or(input))?;
                self.finish(root)
            }
            "projection_to_ir" => {
                let root = self.projection(input.get("projection").unwrap_or(input))?;
                self.finish(root)
            }
            "highlighting_to_ir" => {
                let root = self.highlighting(input.get("highlighting"))?;
                self.finish(root)
            }
            "comparison_to_ir" => {
                let root = self.comparison(input.get("comparison").unwrap_or(input))?;
                self.finish(root)
            }
            "range_to_ir" => {
                let root = self.range(input.get("range").unwrap_or(input))?;
                self.finish(root)
            }
            "filters_to_ir" => {
                let mut request = input.as_object().cloned().unwrap_or_default();
                request.insert("match_all".to_owned(), JsonValue::Bool(true));
                let root = self.search(&JsonValue::Object(request))?;
                self.finish(root)
            }
            "filter_to_ir" => {
                let root = self.filter(input.get("filter").unwrap_or(input))?;
                self.finish(root)
            }
            "sort_to_ir" => {
                let root = self.sort(input)?;
                self.finish(root)
            }
            "facet_to_ir" => Err("facet_to_ir is handled directly by the corpus runner".to_owned()),
            "ranking_to_ir" => {
                let root = self.ranking(input)?;
                self.finish(root)
            }
            "query_to_ir" => {
                let root = self.query_expr(input.get("query_expr").unwrap_or(input))?;
                self.finish(root)
            }
            "search_to_ir" => {
                let root = self.search(input)?;
                self.finish(root)
            }
            "analyze_to_ir" => {
                let root = self.analyze(input)?;
                self.finish(root)
            }
            "insert_to_ir" | "upsert_to_ir" => {
                let document = self.document(input.get("document").unwrap_or(input))?;
                if operation == "insert_to_ir" {
                    let root = api::InsertRequest::create(
                        &mut self.builder,
                        &api::InsertRequestArgs {
                            document: Some(document),
                        },
                    );
                    self.finish(root)
                } else {
                    let condition = input
                        .get("condition")
                        .and_then(JsonValue::as_str)
                        .map(|value| self.text(value));
                    let root = api::UpsertRequest::create(
                        &mut self.builder,
                        &api::UpsertRequestArgs {
                            document: Some(document),
                            condition,
                        },
                    );
                    self.finish(root)
                }
            }
            "update_to_ir" => {
                let request = self.update_request(input)?;
                self.finish(request)
            }
            "delete_to_ir" => {
                let id = input
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                let condition = input
                    .get("condition")
                    .and_then(JsonValue::as_str)
                    .map(|value| self.text(value));
                let root = api::DeleteRequest::create(
                    &mut self.builder,
                    &api::DeleteRequestArgs { id, condition },
                );
                self.finish(root)
            }
            "get_to_ir" => {
                let id = input
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                let projection = input
                    .get("projection")
                    .filter(|v| !v.is_null())
                    .map(|v| self.projection(v))
                    .transpose()?;
                let root = api::GetRequest::create(
                    &mut self.builder,
                    &api::GetRequestArgs { id, projection },
                );
                self.finish(root)
            }
            "patch_to_ir" => {
                let operations = input
                    .get("operations")
                    .and_then(JsonValue::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .map(|v| self.patch_operation(v))
                            .collect::<StdResult<Vec<_>, _>>()
                    })
                    .transpose()?
                    .map(|v| self.builder.create_vector(&v));
                let id = input
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.text(v));
                let root = api::PatchRequest::create(
                    &mut self.builder,
                    &api::PatchRequestArgs { id, operations },
                );
                self.finish(root)
            }
            "write_to_ir" => {
                let request = input.get("request").unwrap_or(input);
                match input
                    .get("kind")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("insert")
                {
                    "insert" => {
                        let document = self.document(request.get("document").unwrap_or(request))?;
                        let root = api::InsertRequest::create(
                            &mut self.builder,
                            &api::InsertRequestArgs {
                                document: Some(document),
                            },
                        );
                        self.finish(root)
                    }
                    "upsert" => {
                        let document = self.document(request.get("document").unwrap_or(request))?;
                        let condition = request
                            .get("condition")
                            .and_then(JsonValue::as_str)
                            .map(|value| self.text(value));
                        let root = api::UpsertRequest::create(
                            &mut self.builder,
                            &api::UpsertRequestArgs {
                                document: Some(document),
                                condition,
                            },
                        );
                        self.finish(root)
                    }
                    "delete" => {
                        let id = request
                            .get("id")
                            .and_then(JsonValue::as_str)
                            .map(|v| self.text(v));
                        let condition = request
                            .get("condition")
                            .and_then(JsonValue::as_str)
                            .map(|value| self.text(value));
                        let root = api::DeleteRequest::create(
                            &mut self.builder,
                            &api::DeleteRequestArgs { id, condition },
                        );
                        self.finish(root)
                    }
                    "patch" => {
                        let operations = request
                            .get("operations")
                            .and_then(JsonValue::as_array)
                            .map(|values| {
                                values
                                    .iter()
                                    .map(|v| self.patch_operation(v))
                                    .collect::<StdResult<Vec<_>, _>>()
                            })
                            .transpose()?
                            .map(|v| self.builder.create_vector(&v));
                        let id = request
                            .get("id")
                            .and_then(JsonValue::as_str)
                            .map(|v| self.text(v));
                        let root = api::PatchRequest::create(
                            &mut self.builder,
                            &api::PatchRequestArgs { id, operations },
                        );
                        self.finish(root)
                    }
                    "update" => {
                        let update = self.update_request(request)?;
                        self.finish(update)
                    }
                    kind => Err(format!("unknown write kind {kind}")),
                }
            }
            _ => Err(format!("no wire builder for {operation}")),
        }
    }
}
