use searchkernel_ir::{
    Backend, ComparisonIr, ComparisonOp, DocumentId, DocumentIr, FieldName, FieldValueIr, FilterIr,
    GroupingAggregationIr, GroupingAllIr, GroupingExpressionIr, GroupingOperationIr,
    GroupingOutputIr, GroupingRequestIr, HighlightingIr, MissingPlacement, PatchOperationIr,
    PrefixPolicy, ProjectionIr, QueryIr, ResultNode, ScalarValueIr, SearchFieldIr,
    SearchFieldWeight, SearchRequestIr, SearchResult, SortDirection, SortIr, TypoPolicy,
};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_vespa_engine::{
    hardening::{self, Action},
    ConfigCompiler, Document, DocumentUpdate, Engine, FacetRequest, Field, FieldType, FieldValue,
    GroupingAggregation, GroupingExpression, GroupingLevel, GroupingPlan, Query, RankProfile,
    ScalarValue, Schema, SearchRequest, SortField, SortOrder, SummaryRequest,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const SEED: u64 = 0x4d11_2026_0914;
type AnyResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn schema() -> Schema {
    Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("body", FieldType::Text).indexed().summary())
        .field(
            Field::new("stored_verification_id", FieldType::Text)
                .indexed()
                .summary(),
        )
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .fast_search()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(Field::new("price", FieldType::Float).attribute().summary())
        .field(
            Field::new("active", FieldType::Boolean)
                .attribute()
                .summary(),
        )
        .field(
            Field::new("popularity", FieldType::Integer)
                .attribute()
                .summary(),
        )
        .rank_profile(
            RankProfile::new("native_features")
                .first_phase("attribute(popularity) + query(boost)")
                .match_feature("attribute(popularity)")
                .match_feature("query(boost)"),
        )
}

fn prepare_config(path: &Path) -> AnyResult<()> {
    if path.is_dir() {
        return Ok(());
    }
    Ok(ConfigCompiler.compile_to(&schema(), path)?)
}

fn open(config: &Path, data: &Path) -> AnyResult<Engine> {
    prepare_config(config)?;
    fs::create_dir_all(data)?;
    Ok(Engine::open(data, config)?)
}

fn grouping_plan(field: &str) -> GroupingPlan {
    GroupingPlan::new().level(
        GroupingLevel::new(GroupingExpression::attribute(field))
            .aggregation(GroupingAggregation::Count),
    )
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string().replace('\n', " ")
}

fn put(engine: &Engine, id: &str, value: i64) -> AnyResult<()> {
    engine.put(
        Document::new(id)
            .field("title", format!("wireless fox {id}"))
            .field("body", "portable native search workload")
            .field("stored_verification_id", id)
            .field("category", if value % 2 == 0 { "audio" } else { "book" })
            .field("qty", value)
            .field("price", value as f64 + 0.5)
            .field("active", value % 2 == 0)
            .field("popularity", value * 10),
    )?;
    Ok(())
}

fn update(engine: &Engine, id: &str, value: i64) -> AnyResult<()> {
    engine.update(DocumentUpdate::new(id).assign("qty", value))?;
    Ok(())
}

fn search_ids(engine: &Engine, value: i64, limit: u32) -> AnyResult<Vec<String>> {
    Ok(engine
        .search(SearchRequest::new(Query::equals("qty", value)).limit(limit))?
        .hits
        .into_iter()
        .map(|hit| hit.document_id)
        .collect())
}

fn advanced_direct(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = open(config, data)?;
    for (id, category, qty, popularity) in [
        ("advanced-a", "audio", 4_i64, 90_i64),
        ("advanced-b", "audio", 7, 20),
        ("advanced-c", "book", 9, 70),
    ] {
        engine.put(
            Document::new(id)
                .field("title", format!("wireless {category} fox"))
                .field("body", "portable native ranking document")
                .field("category", category)
                .field("qty", qty)
                .field("price", qty as f64 + 0.25)
                .field("active", true)
                .field("popularity", popularity),
        )?;
    }
    let sorted = engine.search(
        SearchRequest::new(Query::term("title", "wireless"))
            .sort(SortField::new("popularity", SortOrder::Descending))
            .summary(SummaryRequest::Fields(vec![
                "title".to_owned(),
                "category".to_owned(),
                "price".to_owned(),
            ]))
            .limit(2),
    )?;
    if sorted.hits.len() != 2 || sorted.hits[0].document_id != "advanced-a" {
        return Err("M8 sort/summary path returned an unexpected order".into());
    }
    let facets = engine.search(
        SearchRequest::new(Query::term("title", "wireless"))
            .facet(FacetRequest::new(
                "category",
                8,
                searchkernel_vespa_engine::GroupingOrder::ValueAscending,
            ))
            .limit(10),
    )?;
    if facets.facets.is_empty() || facets.facets[0].groups.len() < 2 {
        return Err("M9 facet path returned no buckets".into());
    }
    let grouped = engine.group(
        &Query::term("title", "wireless"),
        &grouping_plan("category"),
    )?;
    if grouped.groups.is_empty() {
        return Err("M9 grouping path returned no groups".into());
    }
    let ranked = engine.search(
        SearchRequest::new(Query::term("title", "wireless"))
            .rank_profile("native_features")
            .query_feature("boost", 3.0)
            .rank_property("user", "2.0")
            .limit(10),
    )?;
    if ranked.hits.is_empty() || ranked.hits.iter().any(|hit| !hit.score.is_finite()) {
        return Err("M10 ranking path returned invalid scores".into());
    }
    engine.update(
        DocumentUpdate::new("advanced-b")
            .assign("category", "book")
            .assign("popularity", 120_i64),
    )?;
    let updated = engine.search(
        SearchRequest::new(Query::AttributeEquals {
            field: "category".to_owned(),
            value: searchkernel_vespa_engine::Value::String("book".to_owned()),
        })
        .sort(SortField::new("popularity", SortOrder::Descending))
        .limit(10),
    )?;
    if updated.hits.first().map(|hit| hit.document_id.as_str()) != Some("advanced-b") {
        return Err("attribute mutation did not affect sort/filter state".into());
    }
    engine.close()?;
    println!("advanced direct=m8-m10 sort=1 summary=1 facets=1 grouping=1 ranking=1 attributes=1");
    Ok(())
}

fn expect_result_fault<F>(
    engine: &Engine,
    fault: hardening::ResultFault,
    operation: F,
) -> AnyResult<()>
where
    F: FnOnce() -> AnyResult<()>,
{
    hardening::set_result_fault(engine, fault)?;
    let result = operation();
    hardening::clear_result_fault(engine)?;
    if result.is_ok() {
        return Err(format!("result fault {fault:?} was not surfaced").into());
    }
    Ok(())
}

fn ir_field(name: &str) -> AnyResult<FieldName> {
    Ok(FieldName::parse(name.to_owned())?)
}

fn adapter_document(id: &str, category: &str, qty: i64) -> AnyResult<DocumentIr> {
    let mut fields = BTreeMap::new();
    fields.insert(
        ir_field("title")?,
        FieldValueIr::Single(ScalarValueIr::String(format!(
            "adapter wireless {category}"
        ))),
    );
    fields.insert(
        ir_field("body")?,
        FieldValueIr::Single(ScalarValueIr::String("adapter torture body".to_owned())),
    );
    fields.insert(
        ir_field("category")?,
        FieldValueIr::Single(ScalarValueIr::String(category.to_owned())),
    );
    fields.insert(
        ir_field("qty")?,
        FieldValueIr::Single(ScalarValueIr::Int64(qty)),
    );
    fields.insert(
        ir_field("price")?,
        FieldValueIr::Single(ScalarValueIr::Float64(qty as f64 + 0.5)),
    );
    fields.insert(
        ir_field("active")?,
        FieldValueIr::Single(ScalarValueIr::Bool(true)),
    );
    fields.insert(
        ir_field("popularity")?,
        FieldValueIr::Single(ScalarValueIr::Int64(qty * 10)),
    );
    Ok(DocumentIr::new(DocumentId::parse(id.to_owned())?, fields))
}

fn adapter_facet_grouping() -> GroupingRequestIr {
    // The adapter lowers this canonical grouping shape to Vespa's facet
    // request, preserving the M11 facet scenario without a separate API.
    let field = GroupingExpressionIr::Attribute("category".to_owned());
    let count = GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count));
    GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(field),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Max(Some(8)),
                        GroupingOperationIr::Order(vec![GroupingExpressionIr::Negative(Box::new(
                            count.clone(),
                        ))]),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: count,
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

fn adapter_search() -> AnyResult<SearchRequestIr> {
    Ok(SearchRequestIr::new(
        QueryIr::Text("adapter".to_owned()),
        vec![SearchFieldIr::new(
            ir_field("title")?,
            SearchFieldWeight::new(1.0)?,
        )],
        Some(FilterIr::Comparison(ComparisonIr::new(
            ir_field("category")?,
            ComparisonOp::Eq,
            ScalarValueIr::String("audio".to_owned()),
        ))),
        vec![SortIr::new(
            ir_field("popularity")?,
            SortDirection::Desc,
            MissingPlacement::Last,
        )],
        10,
        0,
        ProjectionIr::new(vec![ir_field("title")?, ir_field("category")?]),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![adapter_facet_grouping()]))
}

fn result_has_non_finite_hit(result: &SearchResult) -> bool {
    let ResultNode::Group(group) = &result.root else {
        return false;
    };
    group.children.iter().any(|child| {
        matches!(
            child,
            ResultNode::Hit(hit) if hit.score.is_some_and(|score| !score.is_finite())
        )
    })
}

fn result_has_category_facet(result: &SearchResult) -> bool {
    let ResultNode::Group(group) = &result.root else {
        return false;
    };
    group.children.iter().any(|child| {
        matches!(
            child,
            ResultNode::Group(facet) if facet.id.as_deref() == Some("category")
                && !facet.children.is_empty()
        )
    })
}

fn adapter_torture(config: &Path, data: &Path) -> AnyResult<()> {
    fs::create_dir_all(data)?;
    let backend = Arc::new(VespaBackend::open(data, config)?);
    backend.insert(searchkernel_ir::InsertRequestIr::new(adapter_document(
        "adapter-a",
        "audio",
        4,
    )?))?;
    backend.upsert(searchkernel_ir::UpsertRequestIr::new(adapter_document(
        "adapter-b",
        "audio",
        7,
    )?))?;
    let result = backend.search(adapter_search()?)?;
    let ResultNode::Group(root) = &result.root else {
        return Err("IR adapter search returned a non-group root".into());
    };
    let hit_count = root
        .children
        .iter()
        .filter(|child| matches!(child, ResultNode::Hit(_)))
        .count();
    if hit_count != 2 || !result_has_category_facet(&result) {
        return Err("IR adapter search/filter/facet path returned incomplete results".into());
    }
    let mut readers = Vec::new();
    for _ in 0..4 {
        let backend = Arc::clone(&backend);
        readers.push(thread::spawn(move || -> AnyResult<()> {
            for _ in 0..100 {
                let result = backend.search(adapter_search()?)?;
                if result_has_non_finite_hit(&result) {
                    return Err("IR adapter returned a non-finite score".into());
                }
            }
            Ok(())
        }));
    }
    for reader in readers {
        reader.join().map_err(|_| "IR adapter reader panicked")??;
    }
    backend.patch(searchkernel_ir::PatchRequestIr::new(
        DocumentId::parse("adapter-b")?,
        vec![PatchOperationIr::Set {
            field: ir_field("popularity")?,
            value: FieldValueIr::Single(ScalarValueIr::Int64(100)),
        }],
    ))?;
    let got = backend.get(searchkernel_ir::GetRequestIr::new(
        DocumentId::parse("adapter-b")?,
        ProjectionIr::new(vec![ir_field("popularity")?]),
    ))?;
    if got.document.is_none() {
        return Err("IR adapter get/patch path lost the document".into());
    }
    let deleted = backend.delete(searchkernel_ir::DeleteRequestIr::new(DocumentId::parse(
        "adapter-a",
    )?))?;
    if !deleted.deleted {
        return Err("IR adapter delete path did not report deletion".into());
    }
    let backend = Arc::try_unwrap(backend).map_err(|_| "IR adapter owners remain")?;
    Backend::close(Box::new(backend))?;
    eprintln!("adapter torture=crud search filter sort facets projection");
    Ok(())
}

fn lifecycle(config: &Path, root: &Path) -> AnyResult<()> {
    for cycle in 0..1_000 {
        let data = root.join(format!("cycle-{cycle}"));
        let engine = open(config, &data)?;
        drop(engine);
    }
    println!("lifecycle cycles=1000");
    Ok(())
}

fn reopen(config: &Path, data: &Path) -> AnyResult<()> {
    let mut expected = BTreeMap::new();
    for cycle in 0..250 {
        let engine = open(config, data)?;
        let id = format!("reopen-{cycle:04}");
        put(&engine, &id, cycle)?;
        expected.insert(id, cycle);
        engine.close()?;

        let engine = open(config, data)?;
        for (id, value) in &expected {
            let hits = search_ids(&engine, *value, 1_000_000)?;
            if !hits.iter().any(|hit| hit == id) {
                panic!("reopen cycle {cycle} lost {id}");
            }
        }
        engine.close()?;
    }
    println!("reopen cycles=250 documents={}", expected.len());
    Ok(())
}

fn malformed(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = Arc::new(open(config, data)?);
    let cases = [
        put(&engine, "", 1).err().map(error_message),
        engine
            .put(Document::new("bad").field("missing", 1))
            .err()
            .map(error_message),
        engine
            .search(SearchRequest::new(Query::equals("missing", 1)).limit(1))
            .err()
            .map(error_message),
        engine
            .search(SearchRequest::new(Query::term("qty", "x")))
            .err()
            .map(error_message),
        engine
            .search(
                SearchRequest::new(Query::term("title", "x")).sort(SortField::ascending("missing")),
            )
            .err()
            .map(error_message),
        engine
            .search(
                SearchRequest::new(Query::term("title", "x"))
                    .summary(SummaryRequest::Fields(vec!["missing".to_owned()])),
            )
            .err()
            .map(error_message),
        engine
            .search(
                SearchRequest::new(Query::term("title", "x")).facet(FacetRequest::new(
                    "missing",
                    4,
                    searchkernel_vespa_engine::GroupingOrder::None,
                )),
            )
            .err()
            .map(error_message),
        engine
            .search(SearchRequest::new(Query::And(Vec::new())))
            .err()
            .map(error_message),
        Query::from_yql("select * from sources * where (qty = 1")
            .err()
            .map(error_message),
    ];
    if cases.iter().any(Option::is_none) {
        panic!("malformed input unexpectedly succeeded");
    }
    let invalid_query = Query::Term {
        field: "missing".to_owned(),
        text: "x".to_owned(),
    };
    if engine.search(SearchRequest::new(invalid_query)).is_ok() {
        panic!("unknown query field unexpectedly succeeded");
    }
    if engine
        .group(&Query::term("title", "x"), &grouping_plan("missing"))
        .is_ok()
    {
        panic!("unknown grouping field unexpectedly succeeded");
    }
    let mut deep = Query::Term {
        field: "qty".to_owned(),
        text: "1".to_owned(),
    };
    for _ in 0..257 {
        deep = Query::Not(Box::new(deep));
    }
    if engine.search(SearchRequest::new(deep)).is_ok() {
        panic!("deep query unexpectedly succeeded");
    }
    let wide = Query::And(
        (0..65_537)
            .map(|_| Query::Equals {
                field: "qty".to_owned(),
                value: 1,
            })
            .collect(),
    );
    if engine.search(SearchRequest::new(wide)).is_ok() {
        panic!("wide query unexpectedly succeeded");
    }
    if Query::from_yql(&"x".repeat(4 * 1024 * 1024 + 1)).is_ok() {
        panic!("oversized YQL unexpectedly succeeded");
    }
    println!("malformed cases=14 controlled=true");
    Ok(())
}

fn failures(config: &Path, data: &Path) -> AnyResult<()> {
    hardening::set_open("open_before_native_create", 1, Action::StdException)?;
    let open_error = match Engine::open(data, config) {
        Ok(_) => panic!("open failpoint must fail"),
        Err(error) => error,
    };
    if open_error.code() != 2 {
        panic!("std exception code {} != 2", open_error.code());
    }
    hardening::clear_open("open_before_native_create")?;

    hardening::set_open("open_after_native_create", 1, Action::StdException)?;
    if Engine::open(data, config).is_ok() {
        panic!("post-create open failpoint must fail");
    }
    hardening::clear_open("open_after_native_create")?;

    let engine = Arc::new(open(config, data)?);
    hardening::set(&engine, "mutation_before_native_call", 1, Action::Error)?;
    if put(&engine, "before", 7).is_ok() {
        panic!("before-call failure unexpectedly succeeded");
    }
    if !search_ids(&engine, 7, 10)?.is_empty() {
        panic!("before-call failure changed state");
    }

    hardening::set(
        &engine,
        "mutation_after_native_call",
        1,
        Action::StdException,
    )?;
    if put(&engine, "after", 8).is_ok() {
        panic!("after-call failure unexpectedly succeeded");
    }
    if !search_ids(&engine, 8, 10)?.iter().any(|id| id == "after") {
        panic!("after-call failure incorrectly rolled back native mutation");
    }

    // Prove independent native mutations actually overlap. Hold mutation A
    // after it has entered/completed the Vespa feed call, then require B to
    // complete while A is still blocked. A wrapper-wide mutation mutex makes
    // this deterministic regression fail because B can never enter its feed.
    hardening::set(&engine, "mutation_after_native_call", 1, Action::Block)?;
    let first_engine = Arc::clone(&engine);
    let first = thread::spawn(move || put(&first_engine, "overlap-a", 21));
    for _ in 0..100 {
        if hardening::is_blocked(&engine, "mutation_after_native_call")? {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    if !hardening::is_blocked(&engine, "mutation_after_native_call")? {
        panic!("first mutation did not reach blocking native feed boundary");
    }
    let second_engine = Arc::clone(&engine);
    let (second_done_tx, second_done_rx) = std::sync::mpsc::channel();
    let second = thread::spawn(move || {
        let result = put(&second_engine, "overlap-b", 22);
        let _ = second_done_tx.send(());
        result
    });
    if second_done_rx.recv_timeout(Duration::from_secs(5)).is_err() {
        hardening::release(&engine, "mutation_after_native_call")?;
        let _ = first.join();
        let _ = second.join();
        panic!("independent mutation was globally serialized behind blocked mutation");
    }
    if !hardening::is_blocked(&engine, "mutation_after_native_call")? {
        panic!("first mutation unexpectedly left blocking feed boundary");
    }
    second.join().map_err(|_| "overlap mutation B panicked")??;
    hardening::release(&engine, "mutation_after_native_call")?;
    first.join().map_err(|_| "overlap mutation A panicked")??;
    if !search_ids(&engine, 21, 10)?.iter().any(|id| id == "overlap-a")
        || !search_ids(&engine, 22, 10)?.iter().any(|id| id == "overlap-b")
    {
        panic!("overlapping mutations did not preserve final native state");
    }

    hardening::set(
        &engine,
        "attribute_mutation_before_native_call",
        1,
        Action::Error,
    )?;
    if update(&engine, "after", 11).is_ok() {
        panic!("attribute before-call failure unexpectedly succeeded");
    }
    hardening::set(
        &engine,
        "attribute_mutation_after_native_call",
        1,
        Action::StdException,
    )?;
    if update(&engine, "after", 12).is_ok() {
        panic!("attribute after-call failure unexpectedly succeeded");
    }
    if search_ids(&engine, 12, 10)?.is_empty() {
        panic!("attribute after-call failure rolled back native mutation");
    }

    let failure_query = Query::Equals {
        field: "qty".to_owned(),
        value: 12,
    };

    for (name, action, expected) in [
        ("search_before_match", Action::StdException, 2),
        ("search_after_match", Action::UnknownException, 2),
        ("result_after_allocation", Action::BadAlloc, 3),
        ("result_before_copy", Action::StdException, 2),
    ] {
        hardening::set(&engine, name, 1, action)?;
        let error = match engine.search(SearchRequest::new(failure_query.clone())) {
            Ok(result) => {
                panic!("failure injection unexpectedly succeeded at {name}: {result:?}")
            }
            Err(error) => error,
        };
        if error.code() != expected {
            panic!(
                "failpoint returned code {}, expected {expected}",
                error.code()
            );
        }
    }
    hardening::set(&engine, "search_after_docsums", 1, Action::StdException)?;
    if engine
        .search(
            SearchRequest::new(failure_query.clone())
                .summary(SummaryRequest::Fields(vec!["title".to_owned()])),
        )
        .is_ok()
    {
        panic!("docsums failure injection unexpectedly succeeded");
    }
    hardening::set(&engine, "group_before_match", 1, Action::StdException)?;
    if engine
        .group(&failure_query.clone(), &grouping_plan("category"))
        .is_ok()
    {
        panic!("group failure injection unexpectedly succeeded");
    }
    hardening::set(&engine, "group_after_match", 1, Action::UnknownException)?;
    if engine
        .group(&failure_query.clone(), &grouping_plan("category"))
        .is_ok()
    {
        panic!("post-match group failure injection unexpectedly succeeded");
    }
    for fault in [
        hardening::ResultFault::NullResult,
        hardening::ResultFault::NullHitId,
        hardening::ResultFault::InvalidHitId,
        hardening::ResultFault::NullSummaryName,
        hardening::ResultFault::InvalidSummaryName,
        hardening::ResultFault::NullSummaryString,
        hardening::ResultFault::InvalidSummaryString,
        hardening::ResultFault::UnknownSummaryKind,
    ] {
        expect_result_fault(&engine, fault, || {
            engine
                .search(
                    SearchRequest::new(Query::term("title", "wireless"))
                        .summary(SummaryRequest::Fields(vec!["title".to_owned()])),
                )
                .map(|_| ())
                .map_err(Into::into)
        })?;
    }
    for fault in [
        hardening::ResultFault::DocumentAccessorError,
        hardening::ResultFault::NullDocumentFieldName,
        hardening::ResultFault::InvalidDocumentFieldName,
        hardening::ResultFault::NullDocumentString,
        hardening::ResultFault::InvalidDocumentString,
        hardening::ResultFault::UnknownDocumentKind,
    ] {
        expect_result_fault(&engine, fault, || {
            engine.get("after").map(|_| ()).map_err(Into::into)
        })?;
    }
    for fault in [
        hardening::ResultFault::NullMatchFeatureName,
        hardening::ResultFault::InvalidMatchFeatureName,
        hardening::ResultFault::DuplicateMatchFeatureName,
        hardening::ResultFault::MatchFeatureValueError,
    ] {
        expect_result_fault(&engine, fault, || {
            engine
                .search(
                    SearchRequest::new(Query::term("title", "wireless"))
                        .rank_profile("native_features")
                        .query_feature("boost", 1.0),
                )
                .map(|_| ())
                .map_err(Into::into)
        })?;
    }
    hardening::set(&engine, "search_before_match", 1, Action::Block)?;
    let blocked_engine = Arc::clone(&engine);
    let blocked_search =
        thread::spawn(move || blocked_engine.search(SearchRequest::new(failure_query.clone())));
    for _ in 0..100 {
        if hardening::is_blocked(&engine, "search_before_match")? {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    if !hardening::is_blocked(&engine, "search_before_match")? {
        panic!("block failpoint did not block a search");
    }
    hardening::release(&engine, "search_before_match")?;
    blocked_search
        .join()
        .map_err(|_| "blocked search panicked")??;
    hardening::set(&engine, "destroy_before_native_release", 1, Action::Error)?;
    drop(Arc::try_unwrap(engine).map_err(|_| "engine owners remain")?);
    println!("failures exception_barrier=true cleanup=true attrs=true grouping=true docsums=true");
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ResourceSnapshot {
    fds: u64,
    threads: u64,
    pss_kib: u64,
    files: u64,
    bytes: u64,
}

fn resource_files(root: &Path) -> AnyResult<Vec<std::fs::DirEntry>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    let mut stack = vec![root.to_owned()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                // FileConfigManager keeps an intentional versioned snapshot
                // for each reopen. Those snapshots are Vespa configuration
                // history, not leaked index/resource files; reopen fidelity
                // is exercised separately by the lifecycle suite.
                if entry.file_name() == "config"
                    && path.file_name().is_some_and(|name| name == "typea")
                {
                    continue;
                }
                stack.push(entry.path());
            } else if entry.file_type()?.is_file() {
                files.push(entry);
            }
        }
    }
    Ok(files)
}

fn resource_snapshot(data: &Path) -> AnyResult<ResourceSnapshot> {
    let fds = fs::read_dir("/proc/self/fd")?.count() as u64;
    let status = fs::read_to_string("/proc/self/status")?;
    let threads = status
        .lines()
        .find_map(|line| {
            line.strip_prefix("Threads:")
                .and_then(|v| v.trim().parse().ok())
        })
        .ok_or("missing /proc/self/status Threads")?;
    let smaps = fs::read_to_string("/proc/self/smaps_rollup")?;
    let pss_kib = smaps
        .lines()
        .find_map(|line| {
            line.strip_prefix("Pss:")
                .and_then(|v| v.split_whitespace().next())
        })
        .ok_or("missing /proc/self/smaps_rollup Pss")?
        .parse()?;
    let files = resource_files(data)?;
    let bytes = files.iter().try_fold(0_u64, |total, entry| {
        Ok::<_, std::io::Error>(total.saturating_add(entry.metadata()?.len()))
    })?;
    Ok(ResourceSnapshot {
        fds,
        threads,
        pss_kib,
        files: files.len() as u64,
        bytes,
    })
}

fn resources(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = open(config, data)?;
    put(&engine, "resource", 1)?;
    search_ids(&engine, 1, 10)?;
    let baseline = resource_snapshot(data)?;
    // Reopen/recovery growth is covered by the dedicated lifecycle suite.
    // Keep this measurement read-only so disk accounting isolates result and
    // request resource retention from Vespa's expected recovery bookkeeping.
    for _ in 0..100 {
        search_ids(&engine, 1, 10)?;
    }
    let final_state = resource_snapshot(data)?;
    engine.close()?;
    println!(
        "{{\"baseline_fds\":{},\"final_fds\":{},\"baseline_threads\":{},\"final_threads\":{},\"baseline_pss_kib\":{},\"final_pss_kib\":{},\"baseline_files\":{},\"final_files\":{},\"baseline_bytes\":{},\"final_bytes\":{}}}",
        baseline.fds,
        final_state.fds,
        baseline.threads,
        final_state.threads,
        baseline.pss_kib,
        final_state.pss_kib,
        baseline.files,
        final_state.files,
        baseline.bytes,
        final_state.bytes
    );
    Ok(())
}

fn next_random(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state
}

fn stress(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = open(config, data)?;
    let mut model = BTreeMap::new();
    let mut state = SEED;
    for operation in 0..100_000u32 {
        let slot = (next_random(&mut state) % 128) as u32;
        let id = format!("stress-{slot:03}");
        match next_random(&mut state) % 4 {
            0 => {
                let value = (next_random(&mut state) % 32) as i32;
                put(&engine, &id, i64::from(value))?;
                model.insert(id, value);
            }
            1 if model.contains_key(&id) => {
                let value = (next_random(&mut state) % 32) as i32;
                update(&engine, &id, i64::from(value))?;
                model.insert(id, value);
            }
            2 if model.remove(&id).is_some() => {
                engine.delete(&id)?;
            }
            _ => {
                let value = (next_random(&mut state) % 32) as i32;
                if model.contains_key(&id) {
                    update(&engine, &id, i64::from(value))?;
                } else {
                    put(&engine, &id, i64::from(value))?;
                }
                model.insert(id, value);
            }
        }
        if operation % 10_000 == 0 {
            for value in 0..32 {
                let mut expected: Vec<_> = model
                    .iter()
                    .filter_map(|(id, actual)| (*actual == value).then_some(id.clone()))
                    .collect();
                expected.sort();
                let mut actual = search_ids(&engine, i64::from(value), 1_000_000)?;
                actual.sort();
                if actual != expected {
                    panic!("seed={SEED} operation={operation} state mismatch");
                }
            }
            let advanced = engine.search(
                SearchRequest::new(Query::term("title", "wireless"))
                    .sort(SortField::descending("popularity"))
                    .summary(SummaryRequest::Fields(vec!["title".to_owned()]))
                    .facet(FacetRequest::new(
                        "category",
                        8,
                        searchkernel_vespa_engine::GroupingOrder::ValueAscending,
                    ))
                    .rank_profile("native_features")
                    .query_feature("boost", 1.0)
                    .limit(32),
            )?;
            if advanced.hits.iter().any(|hit| !hit.score.is_finite()) {
                panic!("seed={SEED} operation={operation} advanced result is non-finite");
            }
            let _ = engine.group(
                &Query::term("title", "wireless"),
                &grouping_plan("category"),
            )?;
        }
    }
    println!(
        "stress seed={SEED} operations=100000 documents={}",
        model.len()
    );
    Ok(())
}

fn torture(config: &Path, root: &Path) -> AnyResult<()> {
    stress(config, &root.join("direct"))?;
    adapter_torture(config, &root.join("adapter"))?;
    println!("torture seed={SEED} lanes=direct,searchkernel-ir-adapter");
    Ok(())
}

fn concurrency(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = Arc::new(open(config, data)?);
    engine.put(
        Document::new("stable-text")
            .field("title", "stable fox")
            .field("stored_verification_id", "stable-text"),
    )?;
    let mut workers = Vec::new();
    for worker in 0..8u32 {
        let engine = Arc::clone(&engine);
        workers.push(thread::spawn(move || -> AnyResult<()> {
            for operation in 0..50_000u32 {
                let slot = (operation / 4) % 100;
                let id = format!("writer-{worker:02}-{slot:03}");
                match operation % 4 {
                    0 => put(&engine, &id, 1)?,
                    1 => update(&engine, &id, 2)?,
                    2 => {
                        engine.delete(&id)?;
                    }
                    _ => put(&engine, &id, 3)?,
                }
            }
            Ok(())
        }));
    }
    let mut readers = Vec::new();
    for _ in 0..4 {
        let engine = Arc::clone(&engine);
        readers.push(thread::spawn(move || -> AnyResult<()> {
            for operation in 0..50_000 {
                let _ = search_ids(&engine, 3, 1_000)?;
                let result = engine.search(SearchRequest::new(Query::term("title", "stable")))?;
                if result
                    .hits
                    .iter()
                    .any(|hit| hit.document_id != "stable-text")
                {
                    panic!("impossible text hit");
                }
                if operation % 32 == 0 {
                    let advanced = engine.search(
                        SearchRequest::new(Query::term("title", "fox"))
                            .sort(SortField::descending("popularity"))
                            .summary(SummaryRequest::Fields(vec![
                                "title".to_owned(),
                                "stored_verification_id".to_owned(),
                            ]))
                            .facet(FacetRequest::new(
                                "category",
                                4,
                                searchkernel_vespa_engine::GroupingOrder::CountDescending,
                            ))
                            .rank_profile("native_features")
                            .query_feature("boost", 1.0)
                            .limit(16),
                    )?;
                    if advanced.hits.iter().any(|hit| !hit.score.is_finite()) {
                        panic!("advanced concurrent search returned a non-finite score");
                    }
                    // A document deleted after ID resolution may have an
                    // empty positional docsum. Any populated verification
                    // value must still belong to the hit carrying it.
                    for hit in &advanced.hits {
                        let expected = FieldValue::Scalar(ScalarValue::String(hit.document_id.clone()));
                        if let Some(actual) = hit.fields.get("stored_verification_id") {
                            if actual != &expected {
                                panic!(
                                    "advanced concurrent search associated summary with the wrong hit: hit={} summary={actual:?}",
                                    hit.document_id
                                );
                            }
                        }
                    }
                    let _ = engine.group(
                        &Query::term("title", "fox"),
                        &grouping_plan("category"),
                    )?;
                }
            }
            Ok(())
        }));
    }
    for worker in workers {
        worker.join().map_err(|_| "mutation worker panicked")??;
    }
    for reader in readers {
        reader.join().map_err(|_| "search worker panicked")??;
    }
    let engine = Arc::try_unwrap(engine).map_err(|_| "engine owners remain")?;
    let final_hits = search_ids(&engine, 3, 1_000_000)?;
    if final_hits.len() != 8 * 100 {
        panic!("final state has {} qty=3 documents", final_hits.len());
    }
    engine.close()?;
    println!("concurrency mutation_workers=8 search_workers=4 operations=800000");
    Ok(())
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let mode = args.next().expect("mode");
    if mode == "compile" {
        let config = PathBuf::from(args.next().expect("config directory"));
        ConfigCompiler
            .compile_to(&schema(), config)
            .expect("compile M11 schema");
        return;
    }
    let config = PathBuf::from(args.next().expect("config directory"));
    let data = PathBuf::from(args.next().expect("data directory"));
    let result = match mode.to_string_lossy().as_ref() {
        "lifecycle" => lifecycle(&config, &data),
        "reopen" => reopen(&config, &data),
        "advanced" => advanced_direct(&config, &data),
        "adapter" => adapter_torture(&config, &data),
        "malformed" => malformed(&config, &data),
        "failures" => failures(&config, &data),
        "resources" => resources(&config, &data),
        "stress" => stress(&config, &data),
        "torture" => torture(&config, &data),
        "concurrency" => concurrency(&config, &data),
        other => Err(format!("unknown mode {other}").into()),
    };
    if let Err(error) = result {
        eprintln!("m11 failure: {error}");
        std::process::exit(1);
    }
}
