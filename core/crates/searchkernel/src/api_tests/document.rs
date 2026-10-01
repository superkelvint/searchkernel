use super::*;

#[test]
fn document_identity_resolution_preserves_public_intent_across_ingress_paths() {
    let cases = [
        (None, None, "searchkernel"),
        (Some("catalog"), None, "catalog"),
        (None, Some("product_document"), "product_document"),
        (
            Some("catalog"),
            Some("product_document"),
            "product_document",
        ),
    ];

    for (name, document_type, expected_identity) in cases {
        let mut definition = IndexDefinition::new().field(Field::text("title").stored(true));
        if let Some(name) = name {
            definition = definition.name(name);
        }
        definition.document_type = document_type.map(str::to_owned);

        let direct = definition_to_ir(&definition).expect("direct Rust schema resolves");
        assert_eq!(direct.document_type(), document_type);
        assert_eq!(direct.resolved_document_type(), expected_identity);

        let bytes = crate::portable_definition::encode_portable_definition(&definition);
        let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("portable schema parses");
        let portable =
            searchkernel_ir::wire::v1::index_definition_to_ir(portable).expect("portable schema resolves");
        assert_eq!(portable, direct);
        assert_eq!(portable.document_type(), document_type);
        assert_eq!(portable.resolved_document_type(), expected_identity);
    }
}

#[test]
fn described_public_index_definition_exposes_document_summaries() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = described
        .document_summaries()
        .iter()
        .map(crate::schema::DocumentSummary::to_ir)
        .collect::<Vec<_>>();
    assert_eq!(actual, canonical.document_summaries());
}

#[test]
fn describe_preserves_document_summaries() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.document_summaries(),
        described.document_summaries()
    );
}
