use searchkernel_vespa_engine::{
    AggregateValue, Aggregation, AttributeBound, Document, DocumentUpdate, Engine, Group,
    GroupingRequest, GroupingResult, Query, Result, SearchRequest, SortField, SummaryRequest,
    Value,
};

fn mutations(engine: &Engine) -> Result<()> {
    let document = Document::new("id:test:typea::1")
        .field("category", "books")
        .field("qty", 7_i64)
        .field("price", 12.5_f64)
        .field("active", true);
    engine.put(document)?;

    let update = DocumentUpdate::new("id:test:typea::1")
        .assign("category", "games")
        .assign("qty", 9_i64)
        .assign("price", 17.5_f64)
        .assign("active", false);
    engine.update(update)?;
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
    let request = SearchRequest::new(queries.remove(0))
        .sort(SortField::ascending("category"))
        .sort(SortField::descending("price"))
        .summary(SummaryRequest::None);
    let _ = engine.search(request)?;
    Ok(())
}

fn grouping(engine: &Engine) -> Result<()> {
    let request = GroupingRequest::new(
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

    let result: GroupingResult = engine.group(&request)?;
    for group in result.groups {
        let Group {
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
