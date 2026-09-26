use searchkernel::{ErrorCode, Field, Index, IndexDefinition};
use searchkernel_testkit::RecordingBackend;
use std::sync::Arc;

#[test]
fn public_rust_textual_filter_rejects_aggregate_list_parameter_amplification() {
    let backend = Arc::new(RecordingBackend::new());
    let index = Index::from_backend(
        IndexDefinition::new().field(Field::keyword("brand").filterable(true)),
        backend,
    )
    .expect("test index");

    let per_list = 1_024usize;
    let aggregate = 16_384usize;
    let references = aggregate / per_list + 1;
    let source = std::iter::repeat_n("brand IN (:brands)", references)
        .collect::<Vec<_>>()
        .join(" OR ");
    let brands = (0..per_list)
        .map(|i| format!("brand-{i}"))
        .collect::<Vec<_>>();

    let error = index
        .browse()
        .filter((source, searchkernel::params! { "brands" => brands }))
        .inspect()
        .expect_err("aggregate list-parameter expansion must be rejected before backend execution");

    assert_eq!(error.code(), ErrorCode::InvalidArgument);
    let detail = &error.details()[0];
    assert_eq!(detail.code, "expression_too_complex");
    assert_eq!(detail.path.as_deref(), Some("filters.textual[0].source"));
    assert_eq!(
        detail.metadata.get("limit").map(String::as_str),
        Some("InValues")
    );
}
