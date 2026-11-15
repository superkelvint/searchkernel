#![cfg(feature = "native")]
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_protocol::{json::request_json_to_flatbuffer, protocol::v1 as wire};
use searchkerneld::{
    search_endpoints::{
        start_public_search, PublicEndpoint, PublicFieldKind, PublicSearchConfig, PublicSort,
    },
    IndexRegistry,
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

async fn public_http(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, serde_json::Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(body).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real pinned consumer prerequisites are owned by ./dev test instantsearch-compat"]
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
                "description": format!("{} {} {}", "An introductory description. ".repeat(40), if i <= 18 { "wireless <img src=x> & headphones" } else { "wired speaker" }, "Long trailing text. ".repeat(40)),
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
            policy: Default::default(),
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
    let mut invalid_sort = endpoints["products"].clone();
    invalid_sort.sort = vec![PublicSort {
        field: "missing_sort_field".into(),
        descending: false,
    }];
    endpoints.insert("invalid_sort".into(), invalid_sort);
    let mut stale = endpoints["products"].clone();
    stale.generation = "obsolete-generation".into();
    endpoints.insert("stale".into(), stale);
    let config = PublicSearchConfig {
        endpoints,
        allowed_origins: vec![],
        behind_rate_limited_proxy: true,
        serving: Default::default(),
    };
    let compiled = config.compile().unwrap();
    let preflight: searchkerneld::search_endpoints::PublicSearchRequest = serde_json::from_str(r#"{"endpoint":"products","query":"","limit":6,"offset":0,"facets":["brand","category"],"facetLimit":100,"filters":[],"highlights":["title"],"snippets":["description"]}"#).unwrap();
    let frame = compiled
        .bind_post(&preflight)
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
        ("products", "wireless headphones", 20, 0),
    ] {
        let request = searchkerneld::search_endpoints::PublicSearchRequest {
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
        let bytes = registry
            .dispatch(&compiled.bind_post(&request).unwrap())
            .unwrap();
        let projection = compiled.project_response(&bytes, &request, 0);
        assert!(
            projection.is_ok(),
            "canonical {name}/{query}/{limit}: {:?}; {}",
            projection,
            String::from_utf8_lossy(
                &searchkernel_protocol::json::response_flatbuffer_to_json(&bytes).unwrap()
            )
        );
    }
    // Resource limits belong to deployment policy; use a second real listener
    // to exercise small response/batch/body ceilings without changing the corpus.
    let mut bounded = config.clone();
    bounded.serving = serde_json::from_value(
        json!({"maxResponseBytes": 64, "maxBatchRequests": 1, "maxBodyBytes": 1024}),
    )
    .unwrap();
    let small_server = start_public_search(
        "127.0.0.1:0".parse().unwrap(),
        bounded.compile().unwrap(),
        registry.clone(),
    )
    .await
    .unwrap();
    let browse = json!({"endpoint":"products", "query":"", "limit":6, "offset":0, "facets":[], "facetLimit":100, "filters":[], "snippets":[]});
    let batch = json!({"searches":[browse.clone()]});
    let (status, body) = public_http(
        small_server.local_addr(),
        "POST",
        "/search",
        &batch.to_string(),
    )
    .await;
    assert_eq!(status, 502);
    assert_eq!(body["error"]["code"], "backend_failure");
    let (status, _) = public_http(
        small_server.local_addr(),
        "GET",
        "/search/products?limit=6&offset=0",
        "",
    )
    .await;
    assert_eq!(status, 502, "GET obeys the same response limit");
    let (status, _) = public_http(
        small_server.local_addr(),
        "POST",
        "/search",
        &json!({"searches":[browse.clone(),browse.clone()]}).to_string(),
    )
    .await;
    assert_eq!(status, 400, "configured batch bound");
    let (status, _) = public_http(
        small_server.local_addr(),
        "POST",
        "/search",
        &" ".repeat(1025),
    )
    .await;
    assert_eq!(status, 400, "configured body bound");
    small_server.shutdown().await;

    let server = start_public_search("127.0.0.1:0".parse().unwrap(), compiled, registry.clone())
        .await
        .expect("isolated public listener");
    let address = server.local_addr();
    let (status, mut get) = public_http(
        address,
        "GET",
        "/search/products?q=wireless&limit=6&offset=0",
        "",
    )
    .await;
    assert_eq!(status, 200);
    let mut query = browse.clone();
    query["query"] = json!("wireless");
    let (status, mut post) = public_http(
        address,
        "POST",
        "/search",
        &json!({"searches":[query.clone()]}).to_string(),
    )
    .await;
    assert_eq!(status, 200);
    get["results"][0]
        .as_object_mut()
        .unwrap()
        .remove("processingTimeMs");
    post["results"][0]
        .as_object_mut()
        .unwrap()
        .remove("processingTimeMs");
    assert_eq!(
        get, post,
        "same real engine result, target, projection and presentation"
    );
    let (status, stale) = public_http(address, "GET", "/search/stale?limit=6&offset=0", "").await;
    assert_eq!(status, 409);
    assert_eq!(stale["error"]["code"], "stale_target");
    for route in [
        "/v1/execute",
        "/admin/status",
        "/searchkernel.v1.SearchKernel/Execute",
        "/search/%70roducts",
        "/search/products/",
    ] {
        for method in ["GET", "POST"] {
            assert_eq!(
                public_http(address, method, route, "{}").await.0,
                404,
                "{method} {route}"
            );
        }
    }
    for raw in [
        "q=%",
        "q=%FF",
        "q=a&q=b",
        "q=%09",
        "limit=4294967296",
        "filter=private",
    ] {
        assert_eq!(
            public_http(address, "GET", &format!("/search/products?{raw}"), "")
                .await
                .0,
            400,
            "{raw}"
        );
    }
    for malformed in ["{", "{\"searches\":[]}", "{\"searches\":null}"] {
        assert_eq!(
            public_http(address, "POST", "/search", malformed).await.0,
            400
        );
    }
    // Invalid batch entries fail before any execution and retain their position.
    let mut invalid = browse.clone();
    invalid["limit"] = json!(101);
    let (status, body) = public_http(
        address,
        "POST",
        "/search",
        &json!({"searches":[browse.clone(),invalid]}).to_string(),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["requestIndex"], 1);
    // Caller-selected subsets must further restrict the endpoint allowlist.
    query["fields"] = json!(["title"]);
    query["highlights"] = json!([]);
    let (status, subset) = public_http(
        address,
        "POST",
        "/search",
        &json!({"searches":[query]}).to_string(),
    )
    .await;
    assert_eq!(status, 200);
    for hit in subset["results"][0]["hits"].as_array().unwrap() {
        assert_eq!(
            hit["fields"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["title"]
        );
        assert!(hit["highlights"].as_object().unwrap().is_empty());
    }
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
