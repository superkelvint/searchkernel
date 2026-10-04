//! Bounded public search representation. Execution always uses canonical frames.
use searchkernel_protocol::json::request_json_to_flatbuffer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicSearchConfig {
    pub endpoints: BTreeMap<String, PublicEndpoint>,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Internet deployment requires TLS termination and request rate limits.
    pub behind_rate_limited_proxy: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicEndpoint {
    pub target: String,
    pub generation: String,
    pub fields: Vec<String>,
    pub filter_fields: BTreeMap<String, PublicFieldKind>,
    pub facets: Vec<String>,
    pub highlights: Vec<String>,
    #[serde(default)]
    pub sort: Vec<PublicSort>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicFieldKind {
    String,
    Bool,
    Int64,
    Float64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicSort {
    pub field: String,
    pub descending: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicSearchRequest {
    pub endpoint: String,
    pub query: String,
    pub limit: u32,
    pub offset: u32,
    pub facets: Vec<String>,
    pub facet_limit: u32,
    pub filters: Vec<PublicFilter>,
    pub fields: Option<Vec<String>>,
    pub highlights: Option<Vec<String>>,
    pub snippets: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum PublicFilter {
    And { clauses: Vec<PublicFilter> },
    Or { clauses: Vec<PublicFilter> },
    Eq { field: String, value: Value },
    Ne { field: String, value: Value },
    Lt { field: String, value: Value },
    Lte { field: String, value: Value },
    Gt { field: String, value: Value },
    Gte { field: String, value: Value },
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PublicSearchError {
    InvalidRequest,
    Unsupported,
    UnknownIndex,
    StaleTarget,
    Unavailable,
    Timeout,
    Overloaded,
    BackendFailure,
}
impl std::fmt::Display for PublicSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Public search error: {self:?}")
    }
}
impl std::error::Error for PublicSearchError {}

impl PublicSearchConfig {
    pub fn validate(&self) -> Result<(), PublicSearchError> {
        if !self.behind_rate_limited_proxy
            || self.endpoints.is_empty()
            || self.endpoints.len() > 64
            || self.allowed_origins.len() > 32
            || self
                .allowed_origins
                .iter()
                .any(|o| o == "*" || o.len() > 1024)
        {
            return Err(PublicSearchError::InvalidRequest);
        }
        for (name, endpoint) in &self.endpoints {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                || searchkernel_ir::validate_index_target(&endpoint.target).is_err()
                || endpoint.generation.is_empty()
                || endpoint.generation.len() > 128
                || endpoint.fields.len() > 64
                || endpoint.sort.len() > 8
                || endpoint.filter_fields.len() > 32
            {
                return Err(PublicSearchError::InvalidRequest);
            }
            bounded_fields(&endpoint.fields)?;
            bounded_fields(&endpoint.facets)?;
            bounded_fields(&endpoint.highlights)?;
            bounded_fields(&endpoint.filter_fields.keys().cloned().collect::<Vec<_>>())?;
            if endpoint
                .facets
                .iter()
                .any(|f| !endpoint.filter_fields.contains_key(f))
                || endpoint
                    .highlights
                    .iter()
                    .any(|f| !endpoint.fields.contains(f))
            {
                return Err(PublicSearchError::InvalidRequest);
            }
        }
        Ok(())
    }

    /// Construct a standard FlatBuffers JSON envelope then use the official
    /// BFBS parser. Field legality/defaults/filter semantics stay in *_to_ir().
    pub fn bind(&self, request: &PublicSearchRequest) -> Result<Vec<u8>, PublicSearchError> {
        let endpoint = self
            .endpoints
            .get(&request.endpoint)
            .ok_or(PublicSearchError::UnknownIndex)?;
        if request.query.len() > 4096
            || request.query.contains('\0')
            || request.limit > 100
            || request.offset > 10000
            || request.facet_limit == 0
            || request.facet_limit > 100
            || request.filters.len() > 32
        {
            return Err(PublicSearchError::InvalidRequest);
        }
        let facets = select_fields(&request.facets, &endpoint.facets)?;
        let fields = select_fields(
            request.fields.as_ref().unwrap_or(&endpoint.fields),
            &endpoint.fields,
        )?;
        let highlights = select_fields(
            request.highlights.as_ref().unwrap_or(&endpoint.highlights),
            &endpoint.highlights,
        )?;
        let snippets = select_fields(&request.snippets, &endpoint.highlights)?;
        let highlight_fields: BTreeSet<_> = highlights.into_iter().chain(snippets).collect();
        let mut nodes = 0;
        let filters = request
            .filters
            .iter()
            .map(|f| lower_filter(f, endpoint, 0, &mut nodes))
            .collect::<Result<Vec<_>, _>>()?;
        let grouping = facets
            .iter()
            .map(|field| facet_grouping(field, request.facet_limit))
            .collect::<Vec<_>>();
        let sort = endpoint
            .sort
            .iter()
            .map(|sort| {
                json!({
                    "target": {"target_type": "FieldSortTarget", "target": {"field": sort.field}},
                    "direction": if sort.descending {"Desc"} else {"Asc"}
                })
            })
            .collect::<Vec<_>>();
        let query = if request.query.trim().is_empty() {
            json!({"source_type": "MatchAllQuery", "source": {}})
        } else {
            json!({"source_type": "SearchKernelTextQuery", "source": {"text": request.query}})
        };
        let mut presentation = json!({"fields": {"fields": fields}});
        if !highlight_fields.is_empty() {
            presentation["highlighting"] = json!({"fields": highlight_fields});
        }
        let envelope = json!({
            "protocol_version": searchkernel_protocol::protocol::v1::PROTOCOL_VERSION,
            "target": {"name": endpoint.target, "generation": endpoint.generation},
            "payload_type": "SearchKernel_Wire_SearchRequest",
            "payload": {"query": query, "limit": {"value": request.limit}, "offset": {"value": request.offset},
                "timeout_ms": {"value": "4000"}, "filters": {"structured": filters}, "sort": sort,
                "grouping": grouping, "presentation": presentation}
        });
        let json = serde_json::to_vec(&envelope).map_err(|_| PublicSearchError::InvalidRequest)?;
        request_json_to_flatbuffer(&json).map_err(|_| PublicSearchError::InvalidRequest)
    }
}

fn bounded_fields(fields: &[String]) -> Result<(), PublicSearchError> {
    if fields.len() > 64
        || fields
            .iter()
            .any(|f| f.is_empty() || f.len() > 128 || f.contains('\0'))
        || fields.iter().collect::<BTreeSet<_>>().len() != fields.len()
    {
        return Err(PublicSearchError::InvalidRequest);
    }
    Ok(())
}
fn select_fields(
    requested: &[String],
    allowed: &[String],
) -> Result<Vec<String>, PublicSearchError> {
    bounded_fields(requested)?;
    if requested == ["*"] {
        return Ok(allowed.to_vec());
    }
    if requested.iter().any(|f| !allowed.contains(f)) {
        return Err(PublicSearchError::Unsupported);
    }
    Ok(requested.to_vec())
}
fn scalar(kind: PublicFieldKind, value: &Value) -> Result<Value, PublicSearchError> {
    let (tag, value) = match kind {
        PublicFieldKind::String => (
            "StringValue",
            json!(value
                .as_str()
                .filter(|v| v.len() <= 1024 && !v.contains('\0'))
                .ok_or(PublicSearchError::InvalidRequest)?),
        ),
        PublicFieldKind::Bool => {
            let parsed = match value {
                Value::Bool(b) => *b,
                Value::String(s) if s == "true" => true,
                Value::String(s) if s == "false" => false,
                _ => return Err(PublicSearchError::InvalidRequest),
            };
            ("BoolValue", json!(parsed))
        }
        PublicFieldKind::Int64 => {
            let number = value
                .as_f64()
                .or_else(|| value.as_str()?.parse::<f64>().ok())
                .ok_or(PublicSearchError::InvalidRequest)?;
            if number.fract() != 0.0 || number.abs() > 9_007_199_254_740_991.0 {
                return Err(PublicSearchError::InvalidRequest);
            }
            ("Int64Value", json!((number as i64).to_string()))
        }
        PublicFieldKind::Float64 => (
            "Float64Value",
            json!(value
                .as_f64()
                .or_else(|| value.as_str()?.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or(PublicSearchError::InvalidRequest)?),
        ),
    };
    Ok(json!({"kind_type": tag, "kind": {"value": value}}))
}
fn lower_filter(
    filter: &PublicFilter,
    endpoint: &PublicEndpoint,
    depth: usize,
    nodes: &mut usize,
) -> Result<Value, PublicSearchError> {
    *nodes += 1;
    if depth > 4 || *nodes > 128 {
        return Err(PublicSearchError::InvalidRequest);
    }
    let (field, value, op) = match filter {
        PublicFilter::And { clauses } | PublicFilter::Or { clauses } => {
            if clauses.is_empty() || clauses.len() > 32 {
                return Err(PublicSearchError::InvalidRequest);
            }
            let children = clauses
                .iter()
                .map(|c| lower_filter(c, endpoint, depth + 1, nodes))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(
                json!({"expr_type": if matches!(filter, PublicFilter::And {..}) {"FilterAnd"} else {"FilterOr"}, "expr": {"clauses": children}}),
            );
        }
        PublicFilter::Eq { field, value } => (field, value, "Eq"),
        PublicFilter::Ne { field, value } => (field, value, "Ne"),
        PublicFilter::Lt { field, value } => (field, value, "Lt"),
        PublicFilter::Lte { field, value } => (field, value, "Lte"),
        PublicFilter::Gt { field, value } => (field, value, "Gt"),
        PublicFilter::Gte { field, value } => (field, value, "Gte"),
    };
    let kind = *endpoint
        .filter_fields
        .get(field)
        .ok_or(PublicSearchError::Unsupported)?;
    Ok(
        json!({"expr_type": "FilterComparison", "expr": {"field": {"path": field}, "op": op, "value": scalar(kind, value)?}}),
    )
}
fn facet_grouping(field: &str, limit: u32) -> Value {
    let attribute = json!({"expr_type": "GroupingAttributeExpression", "expr": {"field": field}});
    let count = json!({"expr_type": "GroupingAggregationExpression", "expr": {"aggregation_type": "CountAggregation", "aggregation": {}}});
    json!({"source_type": "StructuredGroupingSource", "source": {"root": {"label": field, "operations": [
        {"operation_type": "GroupingGroupOperation", "operation": {"expression": attribute}},
        {"operation_type": "GroupingEachOperation", "operation": {"operations": [
            {"operation_type": "GroupingMaxOperation", "operation": {"value_type": "FiniteGroupingMax", "value": {"value": limit}}},
            {"operation_type": "GroupingOrderOperation", "operation": {"expressions": [{"expression": {"expr_type": "GroupingUnaryExpression", "expr": {"op": "Negative", "expression": count}}}]}},
            {"operation_type": "GroupingOutputOperation", "operation": {"outputs": [{"expression": count}]}}
        ]}}
    ]}}})
}

/// Strict projection of canonical scalar/document values. Unsupported result
/// shapes fail instead of exposing engine-specific values or debug formatting.
fn result_value(value: &searchkernel_ir::Value) -> Result<Value, PublicSearchError> {
    use searchkernel_ir::Value as V;
    Ok(match value {
        V::Bool(v) => json!(v),
        V::Int64(v) if v.unsigned_abs() <= 9_007_199_254_740_991 => json!(v),
        V::UInt64(v) if *v <= 9_007_199_254_740_991 => json!(v),
        V::Float64(v) if v.is_finite() => json!(v),
        V::String(v) => json!(v),
        V::Null => Value::Null,
        V::List(values) => Value::Array(values.iter().map(result_value).collect::<Result<_, _>>()?),
        V::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), result_value(value)?)))
                .collect::<Result<_, PublicSearchError>>()?,
        ),
        _ => return Err(PublicSearchError::BackendFailure),
    })
}

pub fn project_response(
    bytes: &[u8],
    request: &PublicSearchRequest,
    endpoint: &PublicEndpoint,
    elapsed_ms: u128,
) -> Result<Value, PublicSearchError> {
    use searchkernel_protocol::protocol::v1 as wire;
    if bytes.len() > 1_048_576 {
        return Err(PublicSearchError::BackendFailure);
    }
    let envelope = flatbuffers::root::<wire::ResponseEnvelope<'_>>(bytes)
        .map_err(|_| PublicSearchError::BackendFailure)?;
    if let Some(error) = envelope.payload_as_search_kernel_wire_error() {
        use searchkernel_protocol::wire::v1::ErrorCode;
        return Err(match error.code() {
            ErrorCode::NotFound => PublicSearchError::UnknownIndex,
            ErrorCode::Conflict => PublicSearchError::StaleTarget,
            ErrorCode::FailedPrecondition
                if error.details().is_some_and(|details| {
                    details.iter().any(|detail| {
                        detail.code() == Some(searchkernel_ir::STALE_INDEX_HANDLE_DETAIL_CODE)
                    })
                }) =>
            {
                PublicSearchError::StaleTarget
            }
            ErrorCode::Unsupported => PublicSearchError::Unsupported,
            ErrorCode::InvalidArgument => PublicSearchError::InvalidRequest,
            ErrorCode::Unavailable => PublicSearchError::Unavailable,
            ErrorCode::ResourceExhausted => PublicSearchError::Overloaded,
            ErrorCode::DeadlineExceeded => PublicSearchError::Timeout,
            _ => PublicSearchError::BackendFailure,
        });
    }
    let result = envelope
        .payload_as_search_kernel_wire_search_result()
        .ok_or(PublicSearchError::BackendFailure)?;
    let result = searchkernel_ir::wire::v1::search_result_from_api(result)
        .map_err(|_| PublicSearchError::BackendFailure)?;
    if !result.errors.is_empty() || result.total_hits.value > 9_007_199_254_740_991 {
        return Err(PublicSearchError::BackendFailure);
    }
    let mut hits = Vec::new();
    let mut facets = BTreeMap::<String, BTreeMap<String, u64>>::new();
    let facet_fields = select_fields(&request.facets, &endpoint.facets)?;
    for field in &facet_fields {
        facets.insert(field.clone(), BTreeMap::new());
    }
    let mut seen_facets = BTreeSet::new();
    collect(
        &result.root,
        endpoint,
        &mut hits,
        &mut facets,
        &mut seen_facets,
    )?;
    if seen_facets.len() != facet_fields.len() {
        return Err(PublicSearchError::BackendFailure);
    }
    let exhaustive_facets = result.total_hits.relation == searchkernel_ir::TotalHitsRelation::Exact
        && facets
            .values()
            .all(|values| values.len() < request.facet_limit as usize);
    Ok(
        json!({"endpoint": request.endpoint, "query": request.query, "hits": hits,
        "totalHits": result.total_hits.value, "exact": result.total_hits.relation == searchkernel_ir::TotalHitsRelation::Exact,
        "processingTimeMs": elapsed_ms, "facets": facets, "exhaustiveFacetsCount": exhaustive_facets}),
    )
}
fn collect(
    node: &searchkernel_ir::ResultNode,
    endpoint: &PublicEndpoint,
    hits: &mut Vec<Value>,
    facets: &mut BTreeMap<String, BTreeMap<String, u64>>,
    seen_facets: &mut BTreeSet<String>,
) -> Result<(), PublicSearchError> {
    match node {
        searchkernel_ir::ResultNode::Hit(hit) => {
            let fields = hit
                .fields
                .iter()
                .filter(|(field, _)| endpoint.fields.contains(field))
                .map(|(field, value)| Ok((field.clone(), result_value(value)?)))
                .collect::<Result<BTreeMap<_, _>, PublicSearchError>>()?;
            let mut highlights = BTreeMap::new();
            for highlight in &hit.highlights {
                if !endpoint.highlights.contains(&highlight.field) {
                    continue;
                }
                highlights.insert(highlight.field.clone(), highlight.fragments.iter().map(|fragment| json!({"text": fragment.text, "matches": fragment.matches.iter().map(|span|
                    json!({"startByte": span.start_byte, "endByte": span.end_byte})).collect::<Vec<_>>()})).collect::<Vec<_>>());
            }
            hits.push(json!({"id": hit.id, "fields": fields, "highlights": highlights}));
        }
        searchkernel_ir::ResultNode::Group(group) => {
            if let Some(counts) = group.label.as_ref().and_then(|label| facets.get_mut(label)) {
                if !seen_facets.insert(
                    group
                        .label
                        .clone()
                        .ok_or(PublicSearchError::BackendFailure)?,
                ) {
                    return Err(PublicSearchError::BackendFailure);
                }
                for child in &group.children {
                    if let searchkernel_ir::ResultNode::Group(child) = child {
                        if let (Some(value), Some(searchkernel_ir::Value::UInt64(count))) =
                            (&child.value, child.fields.get("count"))
                        {
                            let value = result_value(value)?;
                            let name = value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string());
                            counts.insert(name, *count);
                        } else {
                            return Err(PublicSearchError::BackendFailure);
                        }
                    } else {
                        return Err(PublicSearchError::BackendFailure);
                    }
                }
            } else {
                for child in &group.children {
                    collect(child, endpoint, hits, facets, seen_facets)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "native")]
mod serving {
    use super::*;
    use crate::{
        search_endpoints::{await_blocking_with_timeout, SearchEndpointConcurrency},
        IndexRegistry,
    };
    use bytes::Bytes;
    use http::{Request, Response, StatusCode};
    use http_body_util::{BodyExt, Full};
    use hyper::{body::Incoming, service::service_fn};
    use hyper_util::rt::{TokioIo, TokioTimer};
    use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};
    use tokio::{
        net::TcpListener,
        sync::{watch, Semaphore},
        task::JoinHandle,
    };

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Batch {
        searches: Vec<PublicSearchRequest>,
    }

    pub struct PublicSearchServer {
        address: SocketAddr,
        shutdown: watch::Sender<bool>,
        task: JoinHandle<()>,
    }
    impl PublicSearchServer {
        pub fn local_addr(&self) -> SocketAddr {
            self.address
        }
        pub async fn shutdown(self) {
            let _ = self.shutdown.send(true);
            let _ = self.task.await;
        }
    }

    pub async fn start_public_search(
        address: SocketAddr,
        config: PublicSearchConfig,
        registry: Arc<IndexRegistry>,
    ) -> Result<PublicSearchServer, Box<dyn std::error::Error>> {
        config.validate()?;
        let listener = TcpListener::bind(address).await?;
        let address = listener.local_addr()?;
        let config = Arc::new(config);
        let budgets = SearchEndpointConcurrency::new(32, 8)?;
        let connections = Arc::new(Semaphore::new(64));
        let (shutdown, mut stopped) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break; };
                        let Ok(permit) = connections.clone().try_acquire_owned() else { continue; };
                        let (config, registry, budgets) = (config.clone(), registry.clone(), budgets.clone());
                        tasks.spawn(async move {
                            let _permit = permit;
                            let service = service_fn(move |request| handle(request, config.clone(), registry.clone(), budgets.clone()));
                            let mut builder = hyper::server::conn::http1::Builder::new();
                            builder.keep_alive(true).max_headers(32).max_buf_size(16384).timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(2));
                            let _ = tokio::time::timeout(Duration::from_secs(30), builder.serve_connection(TokioIo::new(socket), service)).await;
                        });
                    }
                }
            }
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        });
        Ok(PublicSearchServer {
            address,
            shutdown,
            task,
        })
    }

    fn response(status: StatusCode, value: Value) -> Response<Full<Bytes>> {
        let (status, bytes) = match serde_json::to_vec(&value) {
            Ok(bytes) => (status, bytes),
            Err(_) => (
                StatusCode::BAD_GATEWAY,
                br#"{"error":{"code":"backend_failure","requestIndex":null}}"#.to_vec(),
            ),
        };
        let mut response = Response::new(Full::new(Bytes::from(bytes)));
        *response.status_mut() = status;
        response.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        response.headers_mut().insert(
            http::header::CACHE_CONTROL,
            http::HeaderValue::from_static("no-store"),
        );
        response
    }
    fn error(code: PublicSearchError, index: Option<usize>) -> Response<Full<Bytes>> {
        let status = match code {
            PublicSearchError::UnknownIndex => StatusCode::NOT_FOUND,
            PublicSearchError::StaleTarget => StatusCode::CONFLICT,
            PublicSearchError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            PublicSearchError::Overloaded => StatusCode::TOO_MANY_REQUESTS,
            PublicSearchError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            PublicSearchError::BackendFailure => StatusCode::BAD_GATEWAY,
            _ => StatusCode::BAD_REQUEST,
        };
        response(
            status,
            json!({"error": {"code": code, "requestIndex": index}}),
        )
    }

    async fn handle(
        request: Request<Incoming>,
        config: Arc<PublicSearchConfig>,
        registry: Arc<IndexRegistry>,
        budgets: SearchEndpointConcurrency,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        let origin = request
            .headers()
            .get(http::header::ORIGIN)
            .cloned()
            .filter(|origin| {
                origin.to_str().ok().is_some_and(|origin| {
                    config
                        .allowed_origins
                        .iter()
                        .any(|allowed| allowed == origin)
                })
            });
        let mut response =
            if request.method() == http::Method::OPTIONS && request.uri().path() == "/search" {
                response(StatusCode::NO_CONTENT, Value::Null)
            } else {
                match budgets.try_admit() {
                    Err(_) => error(PublicSearchError::Overloaded, None),
                    Ok(permit) => {
                        let result = tokio::time::timeout(
                            Duration::from_secs(6),
                            process(request, config, registry, budgets),
                        )
                        .await;
                        drop(permit);
                        match result {
                            Ok(response) => response,
                            Err(_) => error(PublicSearchError::Timeout, None),
                        }
                    }
                }
            };
        if let Some(origin) = origin {
            response
                .headers_mut()
                .insert(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            response.headers_mut().insert(
                http::header::ACCESS_CONTROL_ALLOW_METHODS,
                http::HeaderValue::from_static("POST, OPTIONS"),
            );
            response.headers_mut().insert(
                http::header::ACCESS_CONTROL_ALLOW_HEADERS,
                http::HeaderValue::from_static("Content-Type"),
            );
            response
                .headers_mut()
                .insert(http::header::VARY, http::HeaderValue::from_static("Origin"));
        }
        Ok(response)
    }
    async fn process(
        request: Request<Incoming>,
        config: Arc<PublicSearchConfig>,
        registry: Arc<IndexRegistry>,
        budgets: SearchEndpointConcurrency,
    ) -> Response<Full<Bytes>> {
        if request.uri().path() != "/search" || request.uri().query().is_some() {
            return error(PublicSearchError::UnknownIndex, None);
        }
        if request.method() != http::Method::POST
            || request
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                != Some("application/json")
            || request.uri().to_string().len() > 4096
            || request
                .headers()
                .iter()
                .map(|(k, v)| k.as_str().len() + v.as_bytes().len())
                .sum::<usize>()
                > 8192
        {
            return error(PublicSearchError::InvalidRequest, None);
        }
        let mut body = request.into_body();
        let read = async {
            let mut bytes = Vec::new();
            while let Some(frame) = body.frame().await {
                let frame = frame.map_err(|_| PublicSearchError::InvalidRequest)?;
                if let Some(data) = frame.data_ref() {
                    if bytes.len().saturating_add(data.len()) > 65536 {
                        return Err(PublicSearchError::InvalidRequest);
                    }
                    bytes.extend_from_slice(data);
                }
            }
            Ok(bytes)
        };
        let bytes = match tokio::time::timeout(Duration::from_secs(2), read).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(code)) => return error(code, None),
            Err(_) => return error(PublicSearchError::Timeout, None),
        };
        let batch: Batch = match serde_json::from_slice(&bytes) {
            Ok(batch) => batch,
            Err(_) => return error(PublicSearchError::InvalidRequest, None),
        };
        if batch.searches.is_empty() || batch.searches.len() > 16 {
            return error(PublicSearchError::InvalidRequest, None);
        }
        let mut bound = Vec::new();
        for (index, request) in batch.searches.iter().enumerate() {
            match config.bind(request) {
                Ok(bytes) => bound.push(bytes),
                Err(code) => return error(code, Some(index)),
            }
        }
        // One bounded sequential batch occupies a native execution slot until
        // every dispatch exits, even if its HTTP caller times out/disconnects.
        let task = budgets.try_spawn_blocking(move || {
            let mut results = Vec::new();
            let batch_started = std::time::Instant::now();
            for (index, (request, bytes)) in batch.searches.iter().zip(bound).enumerate() {
                if batch_started.elapsed() >= Duration::from_secs(4) {
                    return Err((PublicSearchError::Timeout, index));
                }
                let started = std::time::Instant::now();
                let bytes = registry
                    .dispatch(&bytes)
                    .map_err(|_| (PublicSearchError::Unavailable, index))?;
                let endpoint = config
                    .endpoints
                    .get(&request.endpoint)
                    .ok_or((PublicSearchError::UnknownIndex, index))?;
                let projected =
                    project_response(&bytes, request, endpoint, started.elapsed().as_millis())
                        .map_err(|code| (code, index))?;
                results.push(projected);
            }
            let body = json!({"results": results});
            if serde_json::to_vec(&body)
                .map_err(|_| (PublicSearchError::BackendFailure, 0))?
                .len()
                > 1048576
            {
                return Err((PublicSearchError::BackendFailure, 0));
            }
            Ok(body)
        });
        let Ok(task) = task else {
            return error(PublicSearchError::Overloaded, None);
        };
        match await_blocking_with_timeout(task, Duration::from_secs(4)).await {
            Ok(Ok(body)) => response(StatusCode::OK, body),
            Ok(Err((code, index))) => error(code, Some(index)),
            Err(crate::search_endpoints::SearchEndpointExecutionWaitError::TimedOut) => {
                error(PublicSearchError::Timeout, None)
            }
            Err(_) => error(PublicSearchError::BackendFailure, None),
        }
    }
}
#[cfg(feature = "native")]
pub use serving::{start_public_search, PublicSearchServer};
