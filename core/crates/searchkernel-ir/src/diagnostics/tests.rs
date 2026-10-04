use super::*;

fn request() -> SearchRequestIr {
    SearchRequestIr::new(
        QueryIr::MatchAll,
        vec![],
        None,
        vec![],
        10,
        0,
        ProjectionIr::new(vec![]),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
}
fn execution(id: &str) -> DiagnosticExecution {
    DiagnosticExecution {
        id: id.into(),
        request_hash: "hash".into(),
        target: Some("catalog".into()),
        generation: Some("G1".into()),
        kind: DiagnosticExecutionKind::DiagnosticRerun,
        complete: true,
    }
}
fn result(ids: &[&str]) -> SearchResult {
    let mut result = SearchResult::empty(TotalHits {
        value: ids.len() as u64,
        relation: TotalHitsRelation::Exact,
    });
    result.root = ResultNode::Group(ResultGroup::root(
        ids.iter()
            .map(|id| {
                ResultNode::Hit(Hit {
                    id: (*id).into(),
                    score: Some(1.0),
                    ..Default::default()
                })
            })
            .collect(),
    ));
    result
}

#[test]
fn returned_set_ordinals_preserve_ties_tensors_and_bounds() {
    let mut result = result(&["a", "b", "c"]);
    let ResultNode::Group(group) = &mut result.root else {
        panic!("group")
    };
    for (index, node) in group.children.iter_mut().enumerate() {
        let ResultNode::Hit(hit) = node else {
            panic!("hit")
        };
        hit.match_features.insert(
            "lexical".into(),
            Value::Float64(if index == 2 { 2.0 } else { 4.0 }),
        );
        hit.match_features.insert(
            "embedding".into(),
            Value::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[2])".into(),
                payload: RequestTensorPayloadIr::Dense(vec![1.0, 2.0]),
            }),
        );
    }
    let signals = BTreeSet::from(["lexical".into(), "embedding".into()]);
    let (hits, truncated) = hit_evidence(
        &result,
        &request(),
        &execution("rerun"),
        &DiagnosticLimits::default(),
        &signals,
    );
    assert!(!truncated);
    let lexical = hits[0]
        .signals
        .iter()
        .find(|signal| signal.name == "lexical")
        .unwrap();
    assert_eq!(
        (lexical.position, lexical.ties, lexical.comparison_count),
        (Some(1), Some(2), 3)
    );
    assert_eq!(lexical.comparison_set, "rerun:returned-set");
    assert!(matches!(hits[0].signals[0].value, Value::Tensor(_)));
    let limits = DiagnosticLimits {
        tensor_cells: 2,
        ..Default::default()
    };
    let (hits, truncated) =
        hit_evidence(&result, &request(), &execution("rerun"), &limits, &signals);
    assert!(truncated);
    assert_eq!(
        hits.iter()
            .flat_map(|hit| &hit.signals)
            .filter(|signal| matches!(signal.value, Value::Tensor(_)))
            .count(),
        1
    );
}

#[test]
fn comparison_uses_both_page_offsets_and_incomplete_runs_have_unknown_divergence() {
    let mut request = request();
    request.offset = 20;
    let source = DiagnosticSourceRun {
        execution: execution("original"),
        request,
        result: result(&["a", "b"]),
    };
    let mut rerun = execution("rerun");
    let comparison = compare_runs(&source, &rerun, &result(&["b", "c"]), 30);
    assert_eq!(comparison.overlap_count, 1);
    assert_eq!(comparison.entering, ["c"]);
    assert_eq!(comparison.leaving, ["a"]);
    let movement = comparison
        .movements
        .iter()
        .find(|value| value.id == "b")
        .unwrap();
    assert_eq!(
        (movement.baseline_position, movement.rerun_position),
        (Some(22), Some(31))
    );
    rerun.complete = false;
    assert_eq!(
        compare_runs(&source, &rerun, &result(&[]), 0).results_changed,
        None
    );
}

#[test]
fn exact_variants_are_bounded_backend_rewrites_and_opaque_queries_are_not_parsed() {
    let mut request = request();
    request.query = QueryIr::Typed(Box::new(QueryExprIr {
        kind: QueryExprKindIr::NearestNeighbor {
            document_field: QueryFieldRefIr {
                path: FieldName::parse("embedding").unwrap(),
                element_index: None,
            },
            query_tensor: "q".into(),
        },
        annotations: BTreeMap::from([
            ("approximate".into(), RequestValueIr::Bool(true)),
            ("targetNumHits".into(), RequestValueIr::Int64(10)),
        ]),
    }));
    let original = request.clone();
    let variant = exact_variant(&request, &DiagnosticLimits::default()).unwrap();
    let QueryIr::Typed(expression) = variant.query() else {
        panic!("typed")
    };
    assert_eq!(
        expression.annotations["approximate"],
        RequestValueIr::Bool(false)
    );
    assert_eq!(request, original);
    assert!(exact_variant(
        &request,
        &DiagnosticLimits {
            exact_nn_hits: 5,
            ..Default::default()
        }
    )
    .is_none());
    request.query = QueryIr::Yql {
        source: "select * where nearestNeighbor(embedding,q)".into(),
        parameters: Default::default(),
    };
    assert!(exact_variant(&request, &DiagnosticLimits::default()).is_none());
}

struct EvidenceBackend {
    timeout: bool,
}
impl Backend for EvidenceBackend {
    fn insert(&self, _: InsertRequestIr) -> Result<InsertResult> {
        panic!("diagnostics must not write")
    }
    fn upsert(&self, _: UpsertRequestIr) -> Result<UpsertResult> {
        panic!("diagnostics must not write")
    }
    fn patch(&self, _: PatchRequestIr) -> Result<PatchResult> {
        panic!("diagnostics must not write")
    }
    fn delete(&self, _: DeleteRequestIr) -> Result<DeleteResult> {
        panic!("diagnostics must not write")
    }
    fn analyze(&self, _: AnalyzeRequestIr) -> Result<AnalyzeResult> {
        panic!("not requested")
    }
    fn get(&self, _: GetRequestIr) -> Result<GetResult> {
        Ok(GetResult {
            document: Some(DocumentResult {
                id: "expected".into(),
                fields: Default::default(),
            }),
        })
    }
    fn search(&self, request: SearchRequestIr) -> Result<SearchResult> {
        if self.timeout {
            return Err(PortableError::new(
                crate::ErrorCode::DeadlineExceeded,
                "deadline",
            ));
        }
        Ok(if request.filter().is_some() {
            result(&[])
        } else {
            result(&["expected"])
        })
    }
}
fn context() -> IndexContext {
    let field = FieldIr::new(
        FieldName::parse("in_stock").unwrap(),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
        None,
        FieldCapabilities::new(true, false, true, false, false),
    );
    IndexContext::from_definition_ir(IndexDefinitionIr::new(None, vec![field], vec![]).unwrap())
}
fn input() -> QueryDiagnosticsRequestIr {
    let mut request = request();
    request.filter = Some(FilterIr::Comparison(ComparisonIr::new(
        FieldName::parse("in_stock").unwrap(),
        ComparisonOp::Eq,
        ScalarValueIr::Bool(true),
    )));
    QueryDiagnosticsRequestIr {
        request,
        intent: DiagnosticIntent::MissingDocument,
        requested_evidence: vec![DiagnosticSection::Counterfactual],
        source_run_id: None,
        selected_hits: vec![],
        expected_document_id: Some("expected".into()),
        experiment: None,
        compare_exact: false,
        limits: Default::default(),
    }
}

#[test]
fn missing_and_zero_result_findings_reference_exact_backend_variants_and_unobserved_stages_stay_unknown(
) {
    let response = execute(
        &context(),
        &EvidenceBackend { timeout: false },
        input(),
        None,
        execution("rerun"),
    )
    .unwrap();
    let stages = &response.expected_document.as_ref().unwrap().stages;
    assert_eq!(stages[0].observation, DiagnosticObservation::ObservedYes);
    assert_eq!(stages[1].observation, DiagnosticObservation::Unavailable);
    assert_eq!(stages[5].observation, DiagnosticObservation::ObservedNo);
    let finding = response
        .findings
        .iter()
        .find(|finding| finding.code == "constraint_variant_sufficient")
        .unwrap();
    assert!(finding.message.contains("Removing in_stock = true"));
    for id in &finding.evidence_ids {
        assert!(response.evidence.iter().any(|item| &item.id == id));
    }
    let variant = response
        .evidence
        .iter()
        .find(|item| item.id == finding.evidence_ids[0])
        .unwrap();
    assert!(variant
        .canonical_request
        .as_ref()
        .unwrap()
        .filter()
        .is_none());
    assert_ne!(variant.execution_id, response.execution.id);
    assert!(response
        .evidence
        .iter()
        .any(|item| item.section == DiagnosticSection::Counterfactual
            && item.status == DiagnosticStatus::Unsupported));
}

#[test]
fn timeout_returns_partial_evidence_without_inventing_absence_or_findings() {
    let response = execute(
        &context(),
        &EvidenceBackend { timeout: true },
        input(),
        None,
        execution("rerun"),
    )
    .unwrap();
    assert!(!response.execution.complete);
    assert!(response.findings.is_empty());
    assert!(response
        .expected_document
        .unwrap()
        .stages
        .iter()
        .all(|stage| stage.observation == DiagnosticObservation::Unavailable));
    assert!(response
        .evidence
        .iter()
        .any(|item| item.status == DiagnosticStatus::TimedOut));
}

#[test]
fn friendly_query_comes_from_canonical_operators_without_engine_syntax_guessing() {
    let term = QueryExprIr {
        kind: QueryExprKindIr::Text {
            text: "orchid".into(),
        },
        annotations: Default::default(),
    };
    let expression = QueryExprIr {
        kind: QueryExprKindIr::And(vec![term.clone(), term]),
        annotations: Default::default(),
    };
    assert!(friendly_typed_query(&expression).contains(" AND "));
    let expression = QueryExprIr {
        kind: QueryExprKindIr::NearestNeighbor {
            document_field: QueryFieldRefIr {
                path: FieldName::parse("embedding").unwrap(),
                element_index: None,
            },
            query_tensor: "q".into(),
        },
        annotations: Default::default(),
    };
    assert!(friendly_typed_query(&expression).contains("not exhaustive"));
}
