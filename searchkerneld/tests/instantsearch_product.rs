#![cfg(feature = "native")]
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_protocol::{json::request_json_to_flatbuffer, protocol::v1 as wire};
use searchkerneld::{
    public_search::{
        start_public_search, PublicEndpoint, PublicFieldKind, PublicSearchConfig, PublicSort,
    },
    IndexRegistry,
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real pinned consumer and Chromium prerequisites are owned by ./dev test instantsearch-compat"]
async fn real_pinned_consumer_uses_public_endpoint_and_vespa() {
    let root = std::env::temp_dir().join(format!(
        "instantsearch-product-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    for target in ["products", "other"] {
        let index = Index::create(
            root.join(target),
            IndexDefinition::new()
                .field(Field::text("title").stored(true))
                .field(Field::text("description").stored(true))
                .field(
                    Field::keyword("brand")
                        .stored(true)
                        .filterable(true)
                        .facetable(true),
                )
                .field(
                    Field::keyword("category")
                        .stored(true)
                        .filterable(true)
                        .facetable(true),
                )
                .field(
                    Field::bool("stock")
                        .stored(true)
                        .filterable(true)
                        .facetable(true),
                )
                .field(Field::float64("rating").stored(true).filterable(true))
                .field(
                    Field::int64("price")
                        .stored(true)
                        .filterable(true)
                        .sortable(true),
                ),
        )
        .expect("create real Vespa index");
        // Different cardinalities make a swapped/global target fail acceptance.
        let count = if target == "products" { 24 } else { 12 };
        for i in 1..=count {
            let mut document = json!({
                "title": if i <= 18 { format!("Wireless <angle> & Headphones {i:02}") } else { format!("Wired Speaker {i:02}") },
                "description": format!("{} {} {}", "An introductory description. ".repeat(40), if i <= 18 { "wireless headphones" } else { "wired speaker" }, "Long trailing text. ".repeat(40)),
                "brand": if i % 2 == 1 { "Acme" } else { "Bravo" },
                "category": if i <= 18 { "audio" } else { "home" }, "price": i * 10, "stock": i % 2 == 1, "rating": i as f64 / 10.0
            });
            if i == 24 {
                document.as_object_mut().unwrap().remove("description");
            }
            index
                .upsert(format!("{target}-{i:02}"), &document)
                .expect("populate real index");
        }
        index.close().expect("close seeded index");
    }
    let registry = Arc::new(IndexRegistry::new(&root).unwrap());
    let mut endpoints = BTreeMap::new();
    for target in ["products", "other"] {
        let open = request_json_to_flatbuffer(
            &serde_json::to_vec(&json!({
                "protocol_version": wire::PROTOCOL_VERSION, "target": {"name": target},
                "payload_type": "SearchKernel_Wire_OpenIndexRequest", "payload": {}
            }))
            .unwrap(),
        )
        .unwrap();
        let bytes = registry.dispatch(&open).expect("canonical open index");
        let envelope = flatbuffers::root::<wire::ResponseEnvelope<'_>>(&bytes).unwrap();
        let opened = envelope
            .payload_as_search_kernel_wire_open_index_result()
            .expect("opened result");
        let generation = opened.generation().expect("generation").to_owned();
        let endpoint = PublicEndpoint {
            target: target.to_owned(),
            generation,
            fields: [
                "title",
                "description",
                "brand",
                "category",
                "price",
                "stock",
                "rating",
            ]
            .map(str::to_owned)
            .to_vec(),
            filter_fields: BTreeMap::from([
                ("brand".into(), PublicFieldKind::String),
                ("category".into(), PublicFieldKind::String),
                ("price".into(), PublicFieldKind::Int64),
                ("stock".into(), PublicFieldKind::Bool),
                ("rating".into(), PublicFieldKind::Float64),
            ]),
            facets: ["brand", "category", "stock"].map(str::to_owned).to_vec(),
            highlights: ["title", "description"].map(str::to_owned).to_vec(),
            sort: vec![],
        };
        endpoints.insert(target.to_owned(), endpoint.clone());
        if target == "products" {
            let mut ascending = endpoint.clone();
            ascending.sort = vec![PublicSort {
                field: "price".into(),
                descending: false,
            }];
            endpoints.insert("price_asc".into(), ascending);
            let mut descending = endpoint;
            descending.sort = vec![PublicSort {
                field: "price".into(),
                descending: true,
            }];
            endpoints.insert("price_desc".into(), descending);
        }
    }
    let mut stale = endpoints["products"].clone();
    stale.generation = "obsolete-generation".into();
    endpoints.insert("stale".into(), stale);
    let config = PublicSearchConfig {
        endpoints,
        allowed_origins: vec![],
        behind_rate_limited_proxy: true,
    };
    let preflight: searchkerneld::public_search::PublicSearchRequest = serde_json::from_str(r#"{"endpoint":"products","query":"","limit":6,"offset":0,"facets":["brand","category"],"facetLimit":100,"filters":[],"highlights":["title"],"snippets":["description"]}"#).unwrap();
    let frame = config
        .bind(&preflight)
        .expect("preflight binds canonically");
    let bytes = registry
        .dispatch(&frame)
        .expect("preflight dispatches canonically");
    let envelope = flatbuffers::root::<wire::ResponseEnvelope<'_>>(&bytes).unwrap();
    assert!(
        envelope.payload_as_search_kernel_wire_error().is_none(),
        "canonical preflight: {}",
        String::from_utf8_lossy(
            &searchkernel_protocol::json::response_flatbuffer_to_json(&bytes).unwrap()
        )
    );
    for (name, query, limit, offset) in [
        ("products", "wireless", 3, 3),
        ("other", "speaker", 2, 0),
        ("products", "speaker", 0, 0),
    ] {
        let request = searchkerneld::public_search::PublicSearchRequest {
            endpoint: name.into(),
            query: query.into(),
            limit,
            offset,
            facets: vec![],
            facet_limit: 100,
            filters: vec![],
            fields: None,
            highlights: None,
            snippets: vec![],
        };
        let bytes = registry.dispatch(&config.bind(&request).unwrap()).unwrap();
        let projection = searchkerneld::public_search::project_response(
            &bytes,
            &request,
            &config.endpoints[name],
            0,
        );
        assert!(
            projection.is_ok(),
            "canonical {name}/{query}/{limit}: {:?}; {}",
            projection,
            String::from_utf8_lossy(
                &searchkernel_protocol::json::response_flatbuffer_to_json(&bytes).unwrap()
            )
        );
    }
    let server = start_public_search("127.0.0.1:0".parse().unwrap(), config, registry.clone())
        .await
        .expect("isolated public listener");
    let endpoint = format!("http://{}/search", server.local_addr());
    let package =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../integrations/instantsearch");
    let resolved = std::process::Command::new("bash")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/resolve-node.sh"))
        .arg("npm")
        .output()
        .expect("resolve canonical npm");
    assert!(
        resolved.status.success(),
        "{}",
        String::from_utf8_lossy(&resolved.stderr)
    );
    let npm = String::from_utf8(resolved.stdout)
        .expect("npm path UTF-8")
        .trim()
        .to_owned();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(npm)
            .args(["run", "test:engine"])
            .current_dir(package)
            .env("SEARCHKERNEL_INSTANTSEARCH_ENDPOINT", endpoint)
            .status()
            .expect("run actual pinned consumer acceptance")
    })
    .await
    .unwrap();
    server.shutdown().await;
    registry.close_all().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(status.success(), "real InstantSearch acceptance failed");
}
