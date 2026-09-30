use super::*;

impl WireBuilder {
    pub(in crate::runner) fn comparison(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterComparison<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.query_field(v));
        let op = match input
            .get("op")
            .and_then(JsonValue::as_str)
            .unwrap_or("unspecified")
        {
            "eq" => api::ComparisonOp::Eq,
            "ne" => api::ComparisonOp::Ne,
            "lt" => api::ComparisonOp::Lt,
            "lte" => api::ComparisonOp::Lte,
            "gt" => api::ComparisonOp::Gt,
            "gte" => api::ComparisonOp::Gte,
            _ => api::ComparisonOp::Unspecified,
        };
        let value = input.get("value").map(|v| self.value(v)).transpose()?;
        Ok(api::FilterComparison::create(
            &mut self.builder,
            &api::FilterComparisonArgs { field, op, value },
        ))
    }

    pub(in crate::runner) fn range(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterRange<'static>>, String> {
        let field = input
            .get("field")
            .and_then(JsonValue::as_str)
            .map(|v| self.query_field(v));
        let lower = input
            .get("lower")
            .map(|v| self.range_bound(v))
            .transpose()?;
        let upper = input
            .get("upper")
            .map(|v| self.range_bound(v))
            .transpose()?;
        let include_lower = input
            .get("include_lower")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let include_upper = input
            .get("include_upper")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        Ok(api::FilterRange::create(
            &mut self.builder,
            &api::FilterRangeArgs {
                field,
                lower,
                upper,
                include_lower: include_lower.as_ref(),
                include_upper: include_upper.as_ref(),
            },
        ))
    }

    pub(in crate::runner) fn range_bound(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::RangeBound<'static>>, String> {
        let value = self.value(input)?;
        let finite = api::FiniteRangeBound::create(
            &mut self.builder,
            &api::FiniteRangeBoundArgs { value: Some(value) },
        );
        Ok(api::RangeBound::create(
            &mut self.builder,
            &api::RangeBoundArgs {
                bound_type: api::RangeBoundKind::FiniteRangeBound,
                bound: Some(finite.as_union_value()),
            },
        ))
    }

    pub(in crate::runner) fn filter(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FilterExpr<'static>>, String> {
        let object = input.as_object().ok_or("filter must be an object")?;
        let (expr_type, expr) =
            if let Some(values) = object.get("all").or_else(|| object.get("and")) {
                let values = values
                    .as_array()
                    .ok_or("and must be an array")?
                    .iter()
                    .map(|v| self.filter(v))
                    .collect::<StdResult<Vec<_>, _>>()?;
                let values = self.builder.create_vector(&values);
                (
                    api::FilterExprKind::FilterAnd,
                    api::FilterAnd::create(
                        &mut self.builder,
                        &api::FilterAndArgs {
                            clauses: Some(values),
                        },
                    )
                    .as_union_value(),
                )
            } else if let Some(values) = object.get("any").or_else(|| object.get("or")) {
                let values = values
                    .as_array()
                    .ok_or("or must be an array")?
                    .iter()
                    .map(|v| self.filter(v))
                    .collect::<StdResult<Vec<_>, _>>()?;
                let values = self.builder.create_vector(&values);
                (
                    api::FilterExprKind::FilterOr,
                    api::FilterOr::create(
                        &mut self.builder,
                        &api::FilterOrArgs {
                            clauses: Some(values),
                        },
                    )
                    .as_union_value(),
                )
            } else if let Some(value) = object
                .get("not")
                .or_else(|| object.get("negation").and_then(|v| v.get("clause")))
            {
                let value = self.filter(value)?;
                (
                    api::FilterExprKind::FilterNot,
                    api::FilterNot::create(
                        &mut self.builder,
                        &api::FilterNotArgs {
                            clause: Some(value),
                        },
                    )
                    .as_union_value(),
                )
            } else if let Some(value) = object.get("comparison") {
                (
                    api::FilterExprKind::FilterComparison,
                    self.comparison(value)?.as_union_value(),
                )
            } else if let Some(value) = object.get("range") {
                (
                    api::FilterExprKind::FilterRange,
                    self.range(value)?.as_union_value(),
                )
            } else if let Some(value) = object.get("exists") {
                let field = value
                    .get("field")
                    .and_then(JsonValue::as_str)
                    .map(|v| self.query_field(v));
                (
                    api::FilterExprKind::FilterExists,
                    api::FilterExists::create(&mut self.builder, &api::FilterExistsArgs { field })
                        .as_union_value(),
                )
            } else {
                return Ok(api::FilterExpr::create(
                    &mut self.builder,
                    &api::FilterExprArgs::default(),
                ));
            };
        Ok(api::FilterExpr::create(
            &mut self.builder,
            &api::FilterExprArgs {
                expr_type,
                expr: Some(expr),
            },
        ))
    }
}
