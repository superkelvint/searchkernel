use super::*;

pub(super) fn field(name: &str) -> FieldName {
    FieldName::parse(name).unwrap()
}

pub(super) fn request(
    search_fields: Vec<SearchFieldIr>,
    sort: Vec<SortIr>,
    language: Option<LanguageHint>,
) -> SearchRequestIr {
    SearchRequestIr::new(
        QueryIr::Text("wireless headphones".into()),
        search_fields,
        None,
        sort,
        20,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        language,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
}

pub(super) fn facet_grouping(
    field: &str,
    limit: u32,
    order: searchkernel_vespa_engine::GroupingOrder,
) -> GroupingRequestIr {
    let attribute = || GroupingExpressionIr::Attribute(field.to_owned());
    let count = || GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count));
    let order = match order {
        searchkernel_vespa_engine::GroupingOrder::CountDescending => {
            GroupingExpressionIr::Negative(Box::new(count()))
        }
        searchkernel_vespa_engine::GroupingOrder::CountAscending => count(),
        searchkernel_vespa_engine::GroupingOrder::ValueDescending => {
            GroupingExpressionIr::Negative(Box::new(attribute()))
        }
        searchkernel_vespa_engine::GroupingOrder::ValueAscending => attribute(),
        searchkernel_vespa_engine::GroupingOrder::None => {
            panic!("facet grouping requires an explicit order")
        }
    };
    GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(attribute()),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Max(Some(limit)),
                        GroupingOperationIr::Order(vec![order]),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: count(),
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    }
}

pub(super) fn native_grouping(levels: &[(&str, Vec<GroupingExpressionIr>)]) -> GroupingRequestIr {
    let mut nested = None;
    for (field, outputs) in levels.iter().rev() {
        let output = GroupingOperationIr::Output(
            outputs
                .iter()
                .cloned()
                .map(|expression| GroupingOutputIr {
                    label: None,
                    expression,
                })
                .collect(),
        );
        let mut each_operations = vec![output];
        if let Some(child) = nested {
            each_operations.push(GroupingOperationIr::All(child));
        }
        let each = GroupingAllIr {
            operations: each_operations,
            label: None,
        };
        nested = Some(GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute((*field).to_owned())),
                GroupingOperationIr::Each(each),
            ],
            label: None,
        });
    }
    GroupingRequestIr {
        root: nested.expect("native grouping requires at least one level"),
        continuations: Vec::new(),
    }
}

pub(super) fn contains_group_value(node: &ResultNode, value: &Value) -> bool {
    match node {
        ResultNode::Hit(_) => false,
        ResultNode::Group(group) => {
            group.value.as_ref() == Some(value)
                || group
                    .children
                    .iter()
                    .any(|child| contains_group_value(child, value))
        }
    }
}

pub(super) fn count_expression() -> GroupingExpressionIr {
    GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count))
}

pub(super) fn average_expression(field: &str) -> GroupingExpressionIr {
    GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Avg(
        GroupingExpressionIr::Attribute(field.to_owned()),
    )))
}

pub(super) fn typed_field(name: &str) -> QueryFieldRefIr {
    QueryFieldRefIr {
        path: field(name),
        element_index: None,
    }
}

pub(super) fn typed_expression(kind: QueryExprKindIr) -> QueryIr {
    QueryIr::Typed(Box::new(QueryExprIr {
        kind,
        annotations: std::collections::BTreeMap::new(),
    }))
}

pub(super) fn typed_contains(field_name: &str, kind: QueryExprKindIr) -> QueryIr {
    typed_expression(QueryExprKindIr::Contains {
        field: typed_field(field_name),
        item: Box::new(QueryExprIr {
            kind,
            annotations: std::collections::BTreeMap::new(),
        }),
    })
}

pub(super) fn typed_term(value: &str) -> QueryExprIr {
    QueryExprIr {
        kind: QueryExprKindIr::Term {
            value: RequestValueIr::String(value.to_owned()),
        },
        annotations: std::collections::BTreeMap::new(),
    }
}

pub(super) fn test_document(value: i64) -> searchkernel_ir::DocumentIr {
    searchkernel_ir::DocumentIr::new(
        searchkernel_ir::DocumentId::parse("doc-1").expect("valid document ID"),
        std::collections::BTreeMap::from([(
            field("value"),
            FieldValueIr::Single(ScalarValueIr::Int64(value)),
        )]),
    )
}
