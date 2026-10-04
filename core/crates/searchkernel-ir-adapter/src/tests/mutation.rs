use super::*;

#[cfg(feature = "hardening")]
#[test]
fn delete_waits_for_upsert_source_visibility() {
    use searchkernel_vespa_engine::hardening::{self, Action};
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-upsert-delete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).unwrap();
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .unwrap();
    let backend = Arc::new(VespaBackend::open(&data, &config).unwrap());
    let stage = "mutation_before_visibility_wait";
    hardening::set(&backend.engine, stage, 1, Action::Block).unwrap();
    let writer = Arc::clone(&backend);
    let upsert = std::thread::spawn(move || writer.upsert(UpsertRequestIr::new(test_document(1))));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !hardening::is_blocked(&backend.engine, stage).unwrap() {
        if Instant::now() >= deadline {
            hardening::release(&backend.engine, stage).unwrap();
            let result = upsert.join().unwrap();
            panic!("upsert did not reach visibility barrier: {result:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // The native put has completed, but its source-state wait has not. Delete
    // must not remove that state before the upsert can observe it.
    assert!(
        !backend
            .delete(DeleteRequestIr::new(
                DocumentId::parse("other-doc").unwrap()
            ))
            .unwrap()
            .deleted
    );
    let (started_sender, started_receiver) = mpsc::channel();
    let (done_sender, done_receiver) = mpsc::channel();
    let deleter = Arc::clone(&backend);
    let delete = std::thread::spawn(move || {
        started_sender.send(()).unwrap();
        let result = deleter.delete(DeleteRequestIr::new(DocumentId::parse("doc-1").unwrap()));
        done_sender.send(()).unwrap();
        result
    });
    started_receiver.recv().unwrap();
    let deleted_before_release = done_receiver.recv_timeout(Duration::from_secs(2)).is_ok();
    hardening::release(&backend.engine, stage).unwrap();
    let upsert_result = upsert.join().unwrap();
    let delete_result = delete.join().unwrap();
    let stored = backend.engine.get("doc-1").unwrap();
    hardening::clear(&backend.engine, stage).unwrap();
    let backend = Arc::try_unwrap(backend).unwrap_or_else(|_| panic!("backend still shared"));
    backend.engine.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();

    assert!(
        !deleted_before_release,
        "delete bypassed the upsert visibility barrier"
    );
    assert!(upsert_result.is_ok(), "upsert failed: {upsert_result:?}");
    assert!(delete_result.unwrap().deleted);
    assert!(stored.is_none());
}

#[test]
fn typed_query_b_non_empty_rejects_false_and_documents_alternatives_shape() {
    let error = lower_query(
        &typed_contains(
            "title",
            QueryExprKindIr::NonEmpty(Box::new(QueryExprIr {
                kind: QueryExprKindIr::False,
                annotations: std::collections::BTreeMap::new(),
            })),
        ),
        &[],
    )
    .expect_err("false is an empty native query");
    assert_eq!(error.details[0].code, "empty_typed_query");

    let unsupported = typed_contains(
        "title",
        QueryExprKindIr::Alternatives(vec![QueryAlternativeIr {
            query: QueryExprIr {
                kind: QueryExprKindIr::And(vec![typed_term("wireless"), typed_term("headphones")]),
                annotations: std::collections::BTreeMap::new(),
            },
            exactness: None,
        }]),
    );
    let error = lower_query(&unsupported, &[]).expect_err("alternatives are term-only");
    assert_eq!(
        error.details[0].code,
        "typed_query_alternative_not_supported"
    );
}

#[test]
fn array_string_patch_and_update_reanalyze_each_element() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-array-update-analysis-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("array_update_analysis")
        .field(Field::new("tags", FieldType::ArrayString).indexed());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile array update schema");
    let backend = VespaBackend::open(&data, &config).expect("open array update backend");
    let tags = |values: &[&str]| {
        FieldValueIr::Multi(
            values
                .iter()
                .map(|value| ScalarValueIr::String((*value).to_owned()))
                .collect(),
        )
    };
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:array-update").unwrap(),
            std::collections::BTreeMap::from([(field("tags"), tags(&["quick fox"]))]),
        )))
        .expect("insert array document");

    let request = |term| {
        SearchRequestIr::new(
            typed_expression(QueryExprKindIr::Contains {
                field: typed_field("tags"),
                item: Box::new(typed_term(term)),
            }),
            Vec::new(),
            None,
            Vec::new(),
            10,
            0,
            searchkernel_ir::ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        group
            .children
            .into_iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>()
    };

    backend
        .patch(PatchRequestIr::new(
            DocumentId::parse("doc:array-update").unwrap(),
            vec![PatchOperationIr::Set {
                field: field("tags"),
                value: tags(&["slow hare"]),
            }],
        ))
        .expect("patch array document");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(backend
            .search(request("hare"))
            .expect("search patched term")),
        ["doc:array-update"]
    );
    assert!(ids(backend
        .search(request("quick"))
        .expect("search removed patched term"))
    .is_empty());

    backend
        .update(UpdateRequestIr::new(
            DocumentId::parse("doc:array-update").unwrap(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("tags").unwrap(),
                value: tags(&["bright owl"]),
            }],
        ))
        .expect("update array document");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(backend.search(request("owl")).expect("search updated term")),
        ["doc:array-update"]
    );
    assert!(ids(backend
        .search(request("hare"))
        .expect("search removed updated term"))
    .is_empty());

    backend.engine.close().expect("close array update backend");
    let reopened = VespaBackend::open(&data, &config).expect("reopen array update backend");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(reopened
            .search(request("owl"))
            .expect("search persisted updated term")),
        ["doc:array-update"]
    );
    assert!(ids(reopened
        .search(request("quick"))
        .expect("search persisted removed term"))
    .is_empty());
    reopened
        .engine
        .close()
        .expect("close reopened array update backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn explicit_empty_projection_returns_no_document_fields() {
    let document = Document::new("doc-1")
        .field(
            "title",
            FieldValue::Scalar(ScalarValue::String("fox".into())),
        )
        .field("price", FieldValue::Scalar(ScalarValue::Integer(10)));
    assert!(project_document(&document, &[])
        .expect("empty projection must decode")
        .is_empty());
    assert_eq!(
        project_document(&document, &[field("title")]).expect("title projection must decode"),
        BTreeMap::from([("title".into(), Value::String("fox".into()))])
    );
}

#[test]
fn insert_and_upsert_are_linearizable_for_one_document_id() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-linearizable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let engine = Engine::open(&data, &config).expect("open test engine");
    let probe = std::sync::Arc::new(MutationProbe {
        rendezvous: std::sync::Barrier::new(3),
        release: std::sync::Barrier::new(1),
        update_completed: None,
    });
    let backend = std::sync::Arc::new(VespaBackend::with_mutation_probe(engine, probe.clone()));

    let insert_backend = std::sync::Arc::clone(&backend);
    let insert =
        std::thread::spawn(move || insert_backend.insert(InsertRequestIr::new(test_document(1))));
    let upsert_backend = std::sync::Arc::clone(&backend);
    let upsert =
        std::thread::spawn(move || upsert_backend.upsert(UpsertRequestIr::new(test_document(2))));

    probe.rendezvous.wait();
    assert_eq!(
        insert
            .join()
            .expect("insert thread")
            .map(|result| result.id),
        Ok("doc-1".into())
    );
    assert_eq!(
        upsert
            .join()
            .expect("upsert thread")
            .map(|result| result.id),
        Ok("doc-1".into())
    );

    let stored = backend
        .engine
        .get("doc-1")
        .expect("read final document")
        .expect("final document exists");
    assert_eq!(
        stored.fields().get("value"),
        Some(&searchkernel_vespa_engine::FieldValue::Scalar(
            searchkernel_vespa_engine::ScalarValue::Integer(2),
        ))
    );

    let backend = match std::sync::Arc::try_unwrap(backend) {
        Ok(backend) => backend,
        Err(_) => panic!("test backend still has owners"),
    };
    backend.engine.close().expect("close test engine");
    std::fs::remove_dir_all(root).expect("remove test index");
}

#[test]
fn insert_does_not_overwrite_a_concurrent_create_if_missing_update() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-insert-update-linearizable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let engine = Engine::open(&data, &config).expect("open test engine");
    let (update_completed_sender, update_completed_receiver) = std::sync::mpsc::channel();
    let probe = std::sync::Arc::new(MutationProbe {
        rendezvous: std::sync::Barrier::new(2),
        release: std::sync::Barrier::new(2),
        update_completed: Some(update_completed_sender),
    });
    let backend = std::sync::Arc::new(VespaBackend::with_mutation_probe(engine, probe.clone()));

    let insert_backend = std::sync::Arc::clone(&backend);
    let insert =
        std::thread::spawn(move || insert_backend.insert(InsertRequestIr::new(test_document(1))));
    probe.rendezvous.wait();

    let update_backend = std::sync::Arc::clone(&backend);
    let update = std::thread::spawn(move || {
        update_backend.update(
            UpdateRequestIr::new(
                DocumentId::parse("doc-1").expect("valid document ID"),
                vec![UpdateOperationIr::Assign {
                    target: UpdateTargetIr::parse("value").expect("valid update target"),
                    value: FieldValueIr::Single(ScalarValueIr::Int64(2)),
                }],
            )
            .with_create(true),
        )
    });

    // On the buggy implementation the create-if-missing update completes while
    // insert is paused, after which insert overwrites it. With the fix the
    // update is blocked by the same existence-mutation lock. The timeout is
    // only a bounded observation of this forced interleaving, not a stress race.
    let update_completed_before_insert_release = update_completed_receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .is_ok();
    probe.release.wait();

    let insert_result = insert.join().expect("insert thread");
    let update_result = update.join().expect("update thread");
    let stored = backend
        .engine
        .get("doc-1")
        .expect("read final document")
        .expect("final document exists");
    let stored_value = stored.fields().get("value").cloned();

    if update_completed_before_insert_release {
        assert!(
            insert_result.is_err(),
            "insert succeeded after a create-if-missing update completed"
        );
    } else {
        assert!(
            insert_result.is_ok(),
            "serialized insert should succeed before the create-if-missing update"
        );
    }
    assert!(update_result.is_ok(), "create-if-missing update failed");
    assert_eq!(
        stored_value,
        Some(FieldValue::Scalar(
            searchkernel_vespa_engine::ScalarValue::Integer(2),
        ))
    );

    let backend = match std::sync::Arc::try_unwrap(backend) {
        Ok(backend) => backend,
        Err(_) => panic!("test backend still has owners"),
    };
    backend.engine.close().expect("close test engine");
    std::fs::remove_dir_all(root).expect("remove test index");
}

#[test]
fn non_creating_mutations_leave_a_missing_document_missing() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-noncreating-mutations-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let backend =
        VespaBackend::open(&data, &config).expect("open non-creating mutation test backend");
    let id = DocumentId::parse("missing-doc").expect("valid document ID");

    let _patch_result = backend.patch(PatchRequestIr::new(
        id.clone(),
        vec![PatchOperationIr::Set {
            field: field("value"),
            value: FieldValueIr::Single(ScalarValueIr::Int64(1)),
        }],
    ));
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after patch")
            .is_none(),
        "patch unexpectedly created a missing document"
    );

    let _update_result = backend.update(UpdateRequestIr::new(
        id.clone(),
        vec![UpdateOperationIr::Assign {
            target: UpdateTargetIr::parse("value").expect("valid update target"),
            value: FieldValueIr::Single(ScalarValueIr::Int64(2)),
        }],
    ));
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after ordinary update")
            .is_none(),
        "ordinary update unexpectedly created a missing document"
    );

    let _create_false_result = backend.update(
        UpdateRequestIr::new(
            id.clone(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("value").expect("valid update target"),
                value: FieldValueIr::Single(ScalarValueIr::Int64(3)),
            }],
        )
        .with_create(false),
    );
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after create=false update")
            .is_none(),
        "create=false update unexpectedly created a missing document"
    );

    let deleted = backend
        .delete(DeleteRequestIr::new(id.clone()))
        .expect("delete missing document");
    assert!(
        !deleted.deleted,
        "delete unexpectedly reported a missing document as deleted"
    );
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after delete")
            .is_none(),
        "delete unexpectedly created a missing document"
    );

    backend.engine.close().expect("close test engine");
    std::fs::remove_dir_all(root).expect("remove test index");
}

#[test]
fn session_close_drains_serialized_existence_mutations_without_deadlock() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-existence-close-drain-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let engine = Engine::open(&data, &config).expect("open test engine");
    let probe = std::sync::Arc::new(MutationProbe {
        rendezvous: std::sync::Barrier::new(2),
        release: std::sync::Barrier::new(2),
        update_completed: None,
    });
    let backend = VespaBackend::with_mutation_probe(engine, probe.clone());
    let session = BackendSession::new(Box::new(backend));

    let insert_session = std::sync::Arc::clone(&session);
    let insert = std::thread::spawn(move || {
        insert_session.execute(|backend| backend.insert(InsertRequestIr::new(test_document(1))))
    });
    probe.rendezvous.wait();

    let (update_admitted_sender, update_admitted_receiver) = std::sync::mpsc::channel();
    let update_session = std::sync::Arc::clone(&session);
    let update = std::thread::spawn(move || {
        update_session.execute(|backend| {
            update_admitted_sender
                .send(())
                .expect("signal admitted update");
            backend.update(
                UpdateRequestIr::new(
                    DocumentId::parse("doc-1").expect("valid document ID"),
                    vec![UpdateOperationIr::Assign {
                        target: UpdateTargetIr::parse("value").expect("valid update target"),
                        value: FieldValueIr::Single(ScalarValueIr::Int64(2)),
                    }],
                )
                .with_create(true),
            )
        })
    });
    update_admitted_receiver
        .recv()
        .expect("create-if-missing update must be admitted before close");

    let close_session = std::sync::Arc::clone(&session);
    let closer = std::thread::spawn(move || close_session.close());

    // Observe the CLOSING admission boundary without reaching into session
    // internals. Once new work is rejected, close is definitely draining the
    // two already-admitted operations.
    loop {
        match session.execute(|_| Ok::<(), ()>(())) {
            Err(SessionError::Closed) => break,
            Ok(Ok(())) => std::thread::yield_now(),
            Ok(Err(())) => unreachable!("no-op callback cannot fail"),
        }
    }

    probe.release.wait();

    assert!(matches!(insert.join().expect("insert thread"), Ok(Ok(_))));
    assert!(matches!(update.join().expect("update thread"), Ok(Ok(_))));
    closer
        .join()
        .expect("close thread")
        .expect("session close must drain without deadlock");

    drop(session);
    std::fs::remove_dir_all(root).expect("remove test index");
}
