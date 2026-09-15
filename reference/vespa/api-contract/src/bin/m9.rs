use searchkernel_vespa_engine::{
    AggregateValue, Aggregation, AttributeBound, AttributeDocument, AttributeUpdate, Engine,
    LocalGroup, LocalGroupingRequest, LocalGroupingResult, LocalSearchRequest, Query, Result,
    SortField, SummaryRequest, Value,
};

fn mutations(engine: &Engine) -> Result<()> {
    let document = AttributeDocument::new("id:test:typea::1")
        .field("category", Value::String("books".into()))
        .field("qty", Value::Integer(7))
        .field("price", Value::Float(12.5))
        .field("active", Value::Boolean(true));
    engine.put_attributes(&document)?;

    let update = AttributeUpdate::new("id:test:typea::1")
        .assign("category", Value::String("games".into()))
        .assign("qty", Value::Integer(9))
        .assign("price", Value::Float(17.5))
        .assign("active", Value::Boolean(false));
    engine.update_attributes(&update)?;
    Ok(())
}

fn queries() -> Vec<Query> {
    vec![
        Query::AttributeEquals {
            field: "category".into(),
            value: Value::String("books".into()),
        },
        Query::AttributeEquals {
            field: "active".into(),
            value: Value::Boolean(true),
        },
        Query::AttributeRange {
            field: "qty".into(),
            lower: AttributeBound::Included(Value::Integer(5)),
            upper: AttributeBound::Excluded(Value::Integer(10)),
        },
        Query::AttributeRange {
            field: "price".into(),
            lower: AttributeBound::Unbounded,
            upper: AttributeBound::Included(Value::Float(20.0)),
        },
        Query::Exists {
            field: "qty".into(),
        },
        Query::AttributeRange {
            field: "price".into(),
            lower: AttributeBound::Unbounded,
            upper: AttributeBound::Unbounded,
        },
    ]
}

fn search(engine: &Engine) -> Result<()> {
    let mut queries = queries();
    let request = LocalSearchRequest::new(queries.remove(0))
        .sort_by(SortField::ascending("category"))
        .sort_by(SortField::descending("price"))
        .summary(SummaryRequest::None);
    let _ = engine.search_local(&request)?;
    Ok(())
}

fn grouping(engine: &Engine) -> Result<()> {
    let request = LocalGroupingRequest::new(
        Query::AttributeRange {
            field: "price".into(),
            lower: AttributeBound::Unbounded,
            upper: AttributeBound::Unbounded,
        },
        "category",
    )
    .aggregation(Aggregation::Sum("qty".into()))
    .aggregation(Aggregation::Min("price".into()))
    .aggregation(Aggregation::Max("price".into()))
    .aggregation(Aggregation::Average("price".into()));

    let result: LocalGroupingResult = engine.group_local(&request)?;
    for group in result.groups {
        let LocalGroup {
            key,
            count,
            aggregations,
        } = group;
        let _: Value = key;
        let _: u64 = count;
        for value in aggregations {
            match value {
                AggregateValue::Integer(value) => {
                    let _: i64 = value;
                }
                AggregateValue::Float(value) => {
                    let _: f64 = value;
                }
            }
        }
    }
    Ok(())
}

fn main() {
    let _ = mutations as fn(&Engine) -> Result<()>;
    let _ = queries as fn() -> Vec<Query>;
    let _ = search as fn(&Engine) -> Result<()>;
    let _ = grouping as fn(&Engine) -> Result<()>;
}
