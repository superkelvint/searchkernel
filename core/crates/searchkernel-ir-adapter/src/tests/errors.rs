use super::*;

#[cfg(feature = "hardening")]
#[test]
fn malformed_native_results_are_portable_internal_errors() {
    use searchkernel_vespa_engine::hardening::{self, ResultFault};

    let root = std::env::temp_dir().join(format!(
        "searchkernel-error-domains-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel")
                .field(Field::new("value", FieldType::Integer).attribute().summary()),
            root.join("config"),
        )
        .unwrap();
    let backend = VespaBackend::open(root.join("data"), root.join("config")).unwrap();
    backend.upsert(UpsertRequestIr::new(test_document(7))).unwrap();
    for fault in [
        ResultFault::NullDocumentFieldName,
        ResultFault::InvalidDocumentFieldName,
        ResultFault::UnknownDocumentKind,
    ] {
        hardening::set_result_fault(&backend.engine, fault).unwrap();
        let error = backend
            .get(GetRequestIr::new(
                DocumentId::parse("doc-1").unwrap(),
                ProjectionIr::new(vec![field("value")]),
            ))
            .unwrap_err();
        hardening::clear_result_fault(&backend.engine).unwrap();
        assert_eq!(error.code, ErrorCode::Internal, "{fault:?}: {error}");
        assert!(!error.retryable);
        assert_eq!(error.details.len(), 1);
        assert_eq!(error.details[0].code, "native_result_invalid");
        assert_eq!(error.details[0].path, None);
        assert_eq!(error.details[0].message, error.message);
        assert!(backend.engine.get("doc-1").unwrap().is_some());
    }

    let caller_error = native_error(backend.engine.get("bad\0id").unwrap_err());
    assert_eq!(caller_error.code, ErrorCode::InvalidArgument);
    assert!(!caller_error.retryable);
    assert_eq!(caller_error.details[0].code, "backend_request");
    backend.engine.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
