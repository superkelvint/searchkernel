use super::*;

#[test]
fn presence_sensitive_empty_presentation_controls_do_not_equal_absence() {
    let mut context = context();
    context.definition.fields[0].capabilities.stored = true;

    let absent = resolve_search(&context, filter_search_input(Vec::new(), Vec::new()))
        .expect("absent presentation controls should resolve");
    let mut explicit_input = filter_search_input(Vec::new(), Vec::new());
    explicit_input.projection = Some(Vec::new());
    explicit_input.highlighting = Some(Vec::new());
    let explicit = resolve_search(&context, explicit_input)
        .expect("present-empty presentation controls should resolve");

    assert_ne!(absent.projection(), explicit.projection());
    assert_ne!(absent.highlighting(), explicit.highlighting());
    assert_ne!(
        crate::snapshot::search_request(&absent),
        crate::snapshot::search_request(&explicit)
    );
}

#[test]
fn present_empty_container_controls_survive_resolution_and_own_property_namespaces() {
    for (field, property) in [
        ("query_profile", "queryProfile"),
        ("search_chain", "searchChain"),
    ] {
        let mut input = match_all_search(Vec::new());
        match field {
            "query_profile" => input.query_profile = Some(String::new()),
            "search_chain" => input.search_chain = Some(String::new()),
            _ => unreachable!(),
        }
        let resolved = resolve_search(&context(), input.clone())
            .expect("present-empty control remains represented");
        match field {
            "query_profile" => assert_eq!(resolved.query_profile(), Some("")),
            "search_chain" => assert_eq!(resolved.search_chain(), Some("")),
            _ => unreachable!(),
        }

        input.properties.insert(
            property.to_owned(),
            RequestValueIr::String("generic".to_owned()),
        );
        let error = resolve_search(&context(), input)
            .expect_err("typed present-empty control owns its property namespace");
        assert_eq!(error.details[0].code, "property_overlaps_typed_field");
    }
}

#[test]
fn container_control_presence_matches_direct_and_portable_round_trips() {
    for (field, property) in [
        ("query_profile", "queryProfile"),
        ("search_chain", "searchChain"),
    ] {
        for value in [None, Some(""), Some("profile-or-chain")] {
            let (query_profile, search_chain) = if field == "query_profile" {
                (value, None)
            } else {
                (None, value)
            };
            let bytes = portable_container_control_request(query_profile, search_chain, None);
            let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
            assert_eq!(
                request.query_profile(),
                query_profile,
                "{field} wire presence"
            );
            assert_eq!(
                request.search_chain(),
                search_chain,
                "{field} wire presence"
            );

            let portable = context()
                .search_to_ir(request)
                .expect("portable control resolves");
            let mut direct_input = match_all_search(Vec::new());
            direct_input.query_profile = query_profile.map(ToOwned::to_owned);
            direct_input.search_chain = search_chain.map(ToOwned::to_owned);
            let direct =
                resolve_search(&context(), direct_input.clone()).expect("direct control resolves");
            assert_eq!(portable, direct, "{field} {value:?} direct/portable parity");
            assert_eq!(portable.query_profile(), query_profile);
            assert_eq!(portable.search_chain(), search_chain);

            let encoded = crate::wire::v1::search_request_ir_to_api(&portable);
            let encoded = crate::__flatbuffers::root::<api::SearchRequest>(&encoded).unwrap();
            assert_eq!(encoded.query_profile(), query_profile, "{field} IR-to-API");
            assert_eq!(encoded.search_chain(), search_chain, "{field} IR-to-API");
            assert_eq!(
                context()
                    .search_to_ir(encoded)
                    .expect("IR-to-API-to-IR resolves"),
                portable,
                "{field} {value:?} IR-to-API-to-IR"
            );

            let inspected = crate::wire::v1::inspect_query_result_to_api(&portable);
            let inspected = crate::__flatbuffers::root::<api::InspectQueryResult>(&inspected)
                .expect("resolved request decodes");
            let resolved = inspected
                .resolved_request()
                .expect("resolved request exists");
            assert_eq!(
                resolved.query_profile(),
                query_profile,
                "{field} resolved presence"
            );
            assert_eq!(
                resolved.search_chain(),
                search_chain,
                "{field} resolved presence"
            );

            if value.is_some() {
                direct_input.properties.insert(
                    property.to_owned(),
                    RequestValueIr::String("generic".to_owned()),
                );
                let direct_error = resolve_search(&context(), direct_input)
                    .expect_err("direct typed/property collision is rejected");
                let bytes =
                    portable_container_control_request(query_profile, search_chain, Some(property));
                let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
                let portable_error = context()
                    .search_to_ir(request)
                    .expect_err("portable typed/property collision is rejected");
                assert_eq!(
                    portable_error.details[0].code,
                    "property_overlaps_typed_field"
                );
                assert_same_error(&portable_error, &direct_error);
            }
        }
    }
}
