use searchkernel_vespa_engine::{
    AttributeBound, Document, DocumentUpdate, Engine, GroupingAggregation, GroupingExpression,
    GroupingLevel, GroupingOutput, GroupingPlan, GroupingResult, GroupingResultValue, Query,
    Result, SearchRequest, SortField, SummaryRequest, Value,
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

fn visit_group(result: GroupingResult) {
    let _: Option<GroupingResultValue> = result.key;
    for output in result.outputs {
        match output {
            GroupingOutput::Value(GroupingResultValue::String(value)) => {
                let _: String = value;
            }
            GroupingOutput::Value(GroupingResultValue::Integer(value)) => {
                let _: i64 = value;
            }
            GroupingOutput::Value(GroupingResultValue::Float(value)) => {
                let _: f64 = value;
            }
            GroupingOutput::Value(GroupingResultValue::Boolean(value)) => {
                let _: bool = value;
            }
            GroupingOutput::Value(GroupingResultValue::Quantiles(values)) => {
                let _: Vec<(f64, f64)> = values;
            }
            GroupingOutput::HitList(hits) => {
                let _: Vec<String> = hits;
            }
        }
    }
    for child in result.groups {
        visit_group(child);
    }
}

fn grouping(engine: &Engine) -> Result<()> {
    let plan = GroupingPlan::new().level(
        GroupingLevel::new(GroupingExpression::attribute("category"))
            .aggregation(GroupingAggregation::Count)
            .aggregation(GroupingAggregation::Average(GroupingExpression::attribute("price"))),
    );
    let result = engine.group(
        &Query::AttributeRange {
            field: "price".into(),
            lower: AttributeBound::Unbounded,
            upper: AttributeBound::Unbounded,
        },
        &plan,
    )?;
    visit_group(result);
    Ok(())
}

fn main() {
    let _ = mutations as fn(&Engine) -> Result<()>;
    let _ = queries as fn() -> Vec<Query>;
    let _ = search as fn(&Engine) -> Result<()>;
    let _ = grouping as fn(&Engine) -> Result<()>;
}
