use super::*;

impl WireBuilder {
    pub(in crate::runner) fn search_field(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchField<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let weight = input
            .get("weight")
            .map(parse_float)
            .transpose()?
            .map(api::OptionalDouble::new);
        Ok(api::SearchField::create(
            &mut self.builder,
            &api::SearchFieldArgs {
                field,
                weight: weight.as_ref(),
            },
        ))
    }

    pub(in crate::runner) fn projection(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FieldProjection<'static>>, String> {
        let fields = input
            .get("fields")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| self.text(v.as_str().unwrap_or_default()))
                    .collect::<Vec<_>>()
            })
            .map(|values| self.builder.create_vector(&values));
        Ok(api::FieldProjection::create(
            &mut self.builder,
            &api::FieldProjectionArgs { fields },
        ))
    }

    pub(in crate::runner) fn highlighting(
        &mut self,
        input: Option<&JsonValue>,
    ) -> StdResult<flatbuffers::WIPOffset<api::HighlightRequest<'static>>, String> {
        self.projection(input.unwrap_or(&JsonValue::Null))
            .map(|projection| flatbuffers::WIPOffset::new(projection.value()))
    }

    pub(in crate::runner) fn sort(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SortExpr<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let direction = match input
            .get("direction")
            .and_then(JsonValue::as_str)
            .unwrap_or("unspecified")
        {
            "asc" => api::SortDirection::Asc,
            "desc" => api::SortDirection::Desc,
            _ => api::SortDirection::Unspecified,
        };
        let missing = input.get("missing").and_then(JsonValue::as_str).map(|v| {
            let (policy_type, policy) = match v {
                "first" => {
                    let policy =
                        api::MissingFirst::create(&mut self.builder, &api::MissingFirstArgs {});
                    (
                        api::MissingPolicyKind::MissingFirst,
                        Some(policy.as_union_value()),
                    )
                }
                "last" => {
                    let policy =
                        api::MissingLast::create(&mut self.builder, &api::MissingLastArgs {});
                    (
                        api::MissingPolicyKind::MissingLast,
                        Some(policy.as_union_value()),
                    )
                }
                "unspecified" => (api::MissingPolicyKind(0), None),
                _ => (api::MissingPolicyKind(0), None),
            };
            api::MissingPolicy::create(
                &mut self.builder,
                &api::MissingPolicyArgs {
                    policy_type,
                    policy,
                },
            )
        });
        let target = field.map(|field| {
            let field = api::FieldSortTarget::create(
                &mut self.builder,
                &api::FieldSortTargetArgs { field: Some(field) },
            );
            api::SortTarget::create(
                &mut self.builder,
                &api::SortTargetArgs {
                    target_type: api::SortTargetKind::FieldSortTarget,
                    target: Some(field.as_union_value()),
                },
            )
        });
        Ok(api::SortExpr::create(
            &mut self.builder,
            &api::SortExprArgs {
                target,
                direction,
                missing,
                annotations: None,
            },
        ))
    }

    pub(in crate::runner) fn ranking(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::RankingRequest<'static>>, String> {
        let profile = if input.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            None
        } else {
            input
                .get("profile")
                .and_then(JsonValue::as_str)
                .or_else(|| input.get("bm25").map(|_| "bm25"))
                .or(Some("default"))
        };
        let profile = profile.map(|value| self.text(value));
        let properties = input
            .get("properties")
            .map(|value| self.string_entries(value))
            .transpose()?;
        Ok(api::RankingRequest::create(
            &mut self.builder,
            &api::RankingRequestArgs {
                profile,
                properties,
            },
        ))
    }

    pub(in crate::runner) fn string_entries(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<api::StringEntry<'static>>>,
        >,
        String,
    > {
        let mut result = Vec::new();
        if let Some(values) = input.as_object() {
            for (key, value) in values {
                let key = self.text(key);
                let value = value.as_str().ok_or("string map values must be strings")?;
                let value = self.text(value);
                result.push(api::StringEntry::create(
                    &mut self.builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
        }
        Ok(self.builder.create_vector(&result))
    }

    pub(in crate::runner) fn query_inputs(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::QueryInput<'static>>>, String> {
        let values = input.as_object().ok_or("inputs must be an object")?;
        values
            .iter()
            .map(|(name, value)| {
                let (input_type, input) =
                    if let Some(expression) = value.get("expression").and_then(JsonValue::as_str) {
                        let expression = self.text(expression);
                        let expression = api::ExpressionQueryInput::create(
                            &mut self.builder,
                            &api::ExpressionQueryInputArgs {
                                expression: Some(expression),
                            },
                        );
                        (
                            api::QueryInputKind::ExpressionQueryInput,
                            expression.as_union_value(),
                        )
                    } else {
                        let value = self.value(value)?;
                        let literal = api::LiteralQueryInput::create(
                            &mut self.builder,
                            &api::LiteralQueryInputArgs { value: Some(value) },
                        );
                        (
                            api::QueryInputKind::LiteralQueryInput,
                            literal.as_union_value(),
                        )
                    };
                let name = self.text(name);
                Ok(api::QueryInput::create(
                    &mut self.builder,
                    &api::QueryInputArgs {
                        name: Some(name),
                        input_type,
                        input: Some(input),
                    },
                ))
            })
            .collect()
    }

    pub(in crate::runner) fn query_model(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::QueryModel<'static>>, String> {
        let input = input.as_object().ok_or("query_model must be an object")?;
        let get_text = |this: &mut Self, key: &str| {
            input
                .get(key)
                .and_then(JsonValue::as_str)
                .map(|value| this.text(value))
        };
        let query_type = if let Some(value) = input.get("type") {
            let value = value
                .as_object()
                .ok_or("query_model.type must be an object")?;
            let is_yql_default = value
                .get("is_yql_default")
                .and_then(JsonValue::as_bool)
                .map(api::OptionalBool::new);
            let composite = value
                .get("composite")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let tokenization = value
                .get("tokenization")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let syntax = value
                .get("syntax")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let profile = value
                .get("profile")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            Some(api::QueryModelType::create(
                &mut self.builder,
                &api::QueryModelTypeArgs {
                    composite,
                    tokenization,
                    syntax,
                    profile,
                    is_yql_default: is_yql_default.as_ref(),
                },
            ))
        } else {
            None
        };
        let sources = input
            .get("sources")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let restrict = input
            .get("restrict")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let search_group = input
            .get("search_group")
            .and_then(JsonValue::as_u64)
            .map(|value| api::OptionalUInt32::new(value as u32));
        let query_string = get_text(self, "query_string");
        let default_index = get_text(self, "default_index");
        let encoding = get_text(self, "encoding");
        let filter = get_text(self, "filter");
        let language = get_text(self, "language");
        let locale = get_text(self, "locale");
        let search_path = get_text(self, "search_path");
        Ok(api::QueryModel::create(
            &mut self.builder,
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
                search_group: search_group.as_ref(),
            },
        ))
    }

    pub(in crate::runner) fn string_vector(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<&'static str>>,
        >,
        String,
    > {
        let values = input.as_array().ok_or("string vector must be an array")?;
        let values = values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(|value| self.text(value))
                    .ok_or("string vector values must be strings")
            })
            .collect::<StdResult<Vec<_>, _>>()?;
        Ok(self.builder.create_vector(&values))
    }

    pub(in crate::runner) fn presentation(
        &mut self,
        input: &JsonValue,
        fallback_projection: Option<flatbuffers::WIPOffset<api::FieldProjection<'static>>>,
        fallback_highlighting: Option<flatbuffers::WIPOffset<api::HighlightRequest<'static>>>,
    ) -> StdResult<flatbuffers::WIPOffset<api::PresentationRequest<'static>>, String> {
        let input = input.as_object().ok_or("presentation must be an object")?;
        let fields = input
            .get("fields")
            .map(|value| self.projection(value))
            .transpose()?
            .or(fallback_projection);
        let highlighting = input
            .get("highlighting")
            .map(|value| self.highlighting(Some(value)))
            .transpose()?
            .or(fallback_highlighting);
        let bolding = input
            .get("bolding")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let timing = input
            .get("timing")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let summary = input
            .get("summary")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let format = input
            .get("format")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        Ok(api::PresentationRequest::create(
            &mut self.builder,
            &api::PresentationRequestArgs {
                summary,
                fields,
                highlighting,
                format,
                bolding: bolding.as_ref(),
                timing: timing.as_ref(),
            },
        ))
    }

    pub(in crate::runner) fn grouping_requests(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::GroupingRequest<'static>>>, String> {
        let values = input.as_array().ok_or("grouping must be an array")?;
        values
            .iter()
            .map(|value| self.grouping_request(value))
            .collect()
    }

    pub(in crate::runner) fn grouping_request(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingRequest<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping request must be an object")?;
        let operations = input
            .get("operations")
            .or_else(|| input.get("root").and_then(|root| root.get("operations")))
            .ok_or("grouping request requires operations")?;
        let root = self.grouping_all(operations, input.get("label"))?;
        let source = api::StructuredGroupingSource::create(
            &mut self.builder,
            &api::StructuredGroupingSourceArgs { root: Some(root) },
        );
        let continuations = input
            .get("continuations")
            .map(|value| self.string_vector(value))
            .transpose()?;
        Ok(api::GroupingRequest::create(
            &mut self.builder,
            &api::GroupingRequestArgs {
                source_type: api::GroupingSourceKind::StructuredGroupingSource,
                source: Some(source.as_union_value()),
                continuations,
            },
        ))
    }

    pub(in crate::runner) fn grouping_all(
        &mut self,
        input: &JsonValue,
        label: Option<&JsonValue>,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingAllOperation<'static>>, String> {
        let values = input
            .as_array()
            .ok_or("grouping operations must be an array")?;
        let operations = values
            .iter()
            .map(|value| self.grouping_operation(value))
            .collect::<StdResult<Vec<_>, _>>()?;
        let operations = self.builder.create_vector(&operations);
        let label = label
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        Ok(api::GroupingAllOperation::create(
            &mut self.builder,
            &api::GroupingAllOperationArgs {
                operations: Some(operations),
                label,
            },
        ))
    }

    pub(in crate::runner) fn grouping_operation(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingOperation<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping operation must be an object")?;
        let (operation_type, operation) = if let Some(value) = input.get("all") {
            (
                api::GroupingOperationKind::GroupingAllOperation,
                self.grouping_all(value.get("operations").unwrap_or(value), value.get("label"))?
                    .as_union_value(),
            )
        } else if let Some(value) = input.get("each") {
            let value = value.as_object().ok_or("grouping each must be an object")?;
            let operations = self.grouping_operations_vector(
                value
                    .get("operations")
                    .ok_or("grouping each requires operations")?,
            )?;
            let label = value
                .get("label")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            (
                api::GroupingOperationKind::GroupingEachOperation,
                api::GroupingEachOperation::create(
                    &mut self.builder,
                    &api::GroupingEachOperationArgs {
                        operations: Some(operations),
                        label,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = input.get("group") {
            let expression = self.grouping_expression(value)?;
            (
                api::GroupingOperationKind::GroupingGroupOperation,
                api::GroupingGroupOperation::create(
                    &mut self.builder,
                    &api::GroupingGroupOperationArgs {
                        expression: Some(expression),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = input.get("max") {
            let value = value.as_u64().ok_or("grouping max must be an integer")? as u32;
            let max = api::FiniteGroupingMax::create(
                &mut self.builder,
                &api::FiniteGroupingMaxArgs { value },
            );
            (
                api::GroupingOperationKind::GroupingMaxOperation,
                api::GroupingMaxOperation::create(
                    &mut self.builder,
                    &api::GroupingMaxOperationArgs {
                        value_type: api::GroupingMaxKind::FiniteGroupingMax,
                        value: Some(max.as_union_value()),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = input.get("output") {
            let values = value.as_array().ok_or("grouping output must be an array")?;
            let outputs = values
                .iter()
                .map(|value| self.grouping_output(value))
                .collect::<StdResult<Vec<_>, _>>()?;
            let outputs = self.builder.create_vector(&outputs);
            (
                api::GroupingOperationKind::GroupingOutputOperation,
                api::GroupingOutputOperation::create(
                    &mut self.builder,
                    &api::GroupingOutputOperationArgs {
                        outputs: Some(outputs),
                    },
                )
                .as_union_value(),
            )
        } else {
            return Err("unsupported grouping operation builder shape".to_owned());
        };
        Ok(api::GroupingOperation::create(
            &mut self.builder,
            &api::GroupingOperationArgs {
                operation_type,
                operation: Some(operation),
            },
        ))
    }

    pub(in crate::runner) fn grouping_operations_vector(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<
                'static,
                flatbuffers::ForwardsUOffset<api::GroupingOperation<'static>>,
            >,
        >,
        String,
    > {
        let values = input
            .as_array()
            .ok_or("grouping operations must be an array")?;
        let values = values
            .iter()
            .map(|value| self.grouping_operation(value))
            .collect::<StdResult<Vec<_>, _>>()?;
        Ok(self.builder.create_vector(&values))
    }

    pub(in crate::runner) fn grouping_output(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingOutput<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping output must be an object")?;
        let expression = input
            .get("expression")
            .or_else(|| input.get("aggregation"))
            .ok_or("grouping output requires expression")?;
        let expression = self.grouping_expression(expression)?;
        let label = input
            .get("label")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::GroupingOutput::create(
            &mut self.builder,
            &api::GroupingOutputArgs {
                label,
                expression: Some(expression),
            },
        ))
    }

    pub(in crate::runner) fn grouping_expression(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::GroupingExpression<'static>>, String> {
        let input = input
            .as_object()
            .ok_or("grouping expression must be an object")?;
        let (expr_type, expr) = if let Some(value) = input.get("attribute") {
            let field = value
                .as_str()
                .ok_or("grouping attribute must be a string")?;
            let field = self.text(field);
            let value = api::GroupingAttributeExpression::create(
                &mut self.builder,
                &api::GroupingAttributeExpressionArgs { field: Some(field) },
            );
            (
                api::GroupingExpressionKind::GroupingAttributeExpression,
                value.as_union_value(),
            )
        } else if input.get("aggregation").and_then(JsonValue::as_str) == Some("count") {
            let count =
                api::CountAggregation::create(&mut self.builder, &api::CountAggregationArgs {});
            let aggregation = api::GroupingAggregationExpression::create(
                &mut self.builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::CountAggregation,
                    aggregation: Some(count.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                aggregation.as_union_value(),
            )
        } else if let Some(value) = input.get("negative") {
            let expression = self.grouping_expression(value)?;
            let value = api::GroupingUnaryExpression::create(
                &mut self.builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Negative,
                    expression: Some(expression),
                },
            );
            (
                api::GroupingExpressionKind::GroupingUnaryExpression,
                value.as_union_value(),
            )
        } else {
            return Err("unsupported grouping expression builder shape".to_owned());
        };
        Ok(api::GroupingExpression::create(
            &mut self.builder,
            &api::GroupingExpressionArgs {
                expr_type,
                expr: Some(expr),
            },
        ))
    }

    pub(in crate::runner) fn query_expr(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::QueryExpr<'static>>, String> {
        let object = input
            .as_object()
            .ok_or("query expression must be an object")?;
        let (expr_type, expr) = if let Some(value) = object.get("term") {
            let field = value
                .get("field")
                .and_then(JsonValue::as_str)
                .ok_or("query term requires a field")?;
            let field = self.query_field(field);
            let value = value.get("value").map(|v| self.value(v)).transpose()?;
            let term = api::TermQuery::create(&mut self.builder, &api::TermQueryArgs { value });
            let item = api::QueryExpr::create(
                &mut self.builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::TermQuery,
                    expr: Some(term.as_union_value()),
                    annotations: None,
                },
            );
            (
                api::QueryExprKind::QueryContains,
                api::QueryContains::create(
                    &mut self.builder,
                    &api::QueryContainsArgs {
                        field: Some(field),
                        item: Some(item),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(values) = object.get("all").or_else(|| object.get("and")) {
            let values = values
                .as_array()
                .ok_or("query and must be an array")?
                .iter()
                .map(|v| self.query_expr(v))
                .collect::<StdResult<Vec<_>, _>>()?;
            let values = self.builder.create_vector(&values);
            (
                api::QueryExprKind::QueryAnd,
                api::QueryAnd::create(
                    &mut self.builder,
                    &api::QueryAndArgs {
                        clauses: Some(values),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(values) = object.get("any").or_else(|| object.get("or")) {
            let values = values
                .as_array()
                .ok_or("query or must be an array")?
                .iter()
                .map(|v| self.query_expr(v))
                .collect::<StdResult<Vec<_>, _>>()?;
            let values = self.builder.create_vector(&values);
            (
                api::QueryExprKind::QueryOr,
                api::QueryOr::create(
                    &mut self.builder,
                    &api::QueryOrArgs {
                        clauses: Some(values),
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object
            .get("not")
            .or_else(|| object.get("negation").and_then(|v| v.get("clause")))
        {
            let value = self.query_expr(value)?;
            (
                api::QueryExprKind::QueryNot,
                api::QueryNot::create(
                    &mut self.builder,
                    &api::QueryNotArgs {
                        clause: Some(value),
                    },
                )
                .as_union_value(),
            )
        } else {
            return Ok(api::QueryExpr::create(
                &mut self.builder,
                &api::QueryExprArgs::default(),
            ));
        };
        Ok(api::QueryExpr::create(
            &mut self.builder,
            &api::QueryExprArgs {
                expr_type,
                expr: Some(expr),
                annotations: None,
            },
        ))
    }

    pub(in crate::runner) fn filter_clause(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterClause<'static>>, String> {
        let source = input
            .get("source")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let bindings = input
            .get("bindings")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        Ok(api::FilterClause::create(
            &mut self.builder,
            &api::FilterClauseArgs { source, bindings },
        ))
    }

    pub(in crate::runner) fn search(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchRequest<'static>>, String> {
        let query_text = input
            .get("query")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let query_expr = input
            .get("query_expr")
            .map(|v| self.query_expr(v))
            .transpose()?;
        let query_source = if input
            .get("match_all")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false)
        {
            let match_all =
                api::MatchAllQuery::create(&mut self.builder, &api::MatchAllQueryArgs {});
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::MatchAllQuery,
                    source: Some(match_all.as_union_value()),
                },
            ))
        } else if let Some(query_expr) = query_expr {
            let typed = api::TypedQuery::create(
                &mut self.builder,
                &api::TypedQueryArgs {
                    query: Some(query_expr),
                },
            );
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::TypedQuery,
                    source: Some(typed.as_union_value()),
                },
            ))
        } else if let Some(text) = query_text {
            let source = api::SearchKernelTextQuery::create(
                &mut self.builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::SearchKernelTextQuery,
                    source: Some(source.as_union_value()),
                },
            ))
        } else {
            // A nullable legacy `query` is the wire spelling of an empty text
            // query. Keep the text-source provenance so the IR applies the
            // same default search-field selection as an explicit empty query.
            let text = self.text("");
            let source = api::SearchKernelTextQuery::create(
                &mut self.builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            Some(api::QuerySource::create(
                &mut self.builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::SearchKernelTextQuery,
                    source: Some(source.as_union_value()),
                },
            ))
        };
        let search_fields = input
            .get("search_fields")
            .map(|v| self.selection(v))
            .transpose()?;
        let structured = input.get("filter").map(|v| self.filter(v)).transpose()?;
        let structured = structured.map(|filter| self.builder.create_vector(&[filter]));
        let textual = input
            .get("textual")
            .map(|values| {
                values
                    .as_array()
                    .ok_or("textual must be an array")?
                    .iter()
                    .map(|value| self.filter_clause(value))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|values| self.builder.create_vector(&values));
        let legacy_sources = input
            .get("sources")
            .map(|values| {
                values
                    .as_array()
                    .ok_or("sources must be an array")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(|value| self.text(value))
                            .ok_or("source must be a string")
                    })
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|values| self.builder.create_vector(&values));
        let legacy_parameters = input
            .get("parameters")
            .map(|values| self.entries(Some(values)))
            .transpose()?;
        let filters = (structured.is_some()
            || textual.is_some()
            || legacy_sources.is_some()
            || legacy_parameters.is_some())
        .then(|| {
            api::FilterRequest::create(
                &mut self.builder,
                &api::FilterRequestArgs {
                    structured,
                    textual,
                    sources: legacy_sources,
                    parameters: legacy_parameters,
                },
            )
        });
        let sorts = input
            .get("sort")
            .and_then(JsonValue::as_array)
            .map(|v| {
                v.iter()
                    .map(|x| self.sort(x))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|v| self.builder.create_vector(&v));
        let ranking = input.get("ranking").map(|v| self.ranking(v)).transpose()?;
        let inputs = input
            .get("inputs")
            .map(|v| self.query_inputs(v))
            .transpose()?
            .map(|v| self.builder.create_vector(&v));
        let limit = input
            .get("limit")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let offset = input
            .get("offset")
            .and_then(JsonValue::as_u64)
            .map(|v| api::OptionalUInt32::new(v as u32));
        let timeout = input
            .get("timeout")
            .and_then(JsonValue::as_u64)
            .map(api::OptionalUInt64::new);
        let projection = input
            .get("projection")
            .map(|v| self.projection(v))
            .transpose()?;
        let highlighting = input
            .get("highlighting")
            .map(|v| self.highlighting(Some(v)))
            .transpose()?;
        let language = input
            .get("language")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let query_model = input
            .get("query_model")
            .map(|value| self.query_model(value))
            .transpose()?;
        let query_model = query_model.or_else(|| {
            language.map(|language| {
                api::QueryModel::create(
                    &mut self.builder,
                    &api::QueryModelArgs {
                        language: Some(language),
                        ..Default::default()
                    },
                )
            })
        });
        let presentation = if let Some(value) = input.get("presentation") {
            Some(self.presentation(value, projection, highlighting)?)
        } else if projection.is_some() || highlighting.is_some() {
            Some(api::PresentationRequest::create(
                &mut self.builder,
                &api::PresentationRequestArgs {
                    fields: projection,
                    highlighting,
                    ..Default::default()
                },
            ))
        } else {
            None
        };
        let properties = input
            .get("properties")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        let grouping = input
            .get("grouping")
            .map(|value| self.grouping_requests(value))
            .transpose()?
            .map(|value| self.builder.create_vector(&value));
        let query_profile = input
            .get("query_profile")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let search_chain = input
            .get("search_chain")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        Ok(api::SearchRequest::create(
            &mut self.builder,
            &api::SearchRequestArgs {
                diagnostic_source: input.get("diagnostic_source").and_then(JsonValue::as_bool).unwrap_or(false),
                query: query_source,
                search_fields,
                filters,
                sort: sorts,
                grouping,
                ranking,
                inputs,
                limit: limit.as_ref(),
                offset: offset.as_ref(),
                timeout_ms: timeout.as_ref(),
                query_model,
                presentation,
                properties,
                query_profile,
                search_chain,
            },
        ))
    }

    pub(in crate::runner) fn analyze(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::AnalyzeRequest<'static>>, String> {
        let (target_type, target) = if input.get("field").is_some() {
            let field = input
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            (
                api::AnalyzeTarget::AnalyzeFieldTarget,
                Some(
                    api::AnalyzeFieldTarget::create(
                        &mut self.builder,
                        &api::AnalyzeFieldTargetArgs { field },
                    )
                    .as_union_value(),
                ),
            )
        } else if input.get("analyzer").is_some() {
            (
                api::AnalyzeTarget::Analyzer,
                Some(self.analyzer(input.get("analyzer"))?.as_union_value()),
            )
        } else {
            (api::AnalyzeTarget::NONE, None)
        };
        let text = input
            .get("text")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let language = input
            .get("language")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::AnalyzeRequest::create(
            &mut self.builder,
            &api::AnalyzeRequestArgs {
                target_type,
                target,
                text,
                language,
            },
        ))
    }
}
