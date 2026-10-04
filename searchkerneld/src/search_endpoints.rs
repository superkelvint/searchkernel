//! Public Search Endpoint request binding for `searchkerneld`.
//!
//! # Status
//!
//! The first two implementation slices are present here: a typed endpoint
//! policy/request model, strict named-route GET decoding, and canonical
//! request-envelope encoding for text and match-all search. Public listener
//! wiring, public JSON response projection, batching, and richer controls remain
//! future slices.
//!
//! InstantSearch.js compatibility is nevertheless a hard acceptance requirement
//! for this request-binding surface. The first compatibility profile accepts the
//! core SearchClient vocabulary directly: `query`, `hitsPerPage`, and
//! zero-based `page`. These are exposure-policy aliases only; they lower to the
//! same canonical Search Kernel query/limit/offset fields and do not introduce
//! Algolia search semantics.
//!
//! Search Endpoints are intended to let `searchkerneld` serve deliberately
//! narrow, user-facing search APIs such as:
//!
//! ```text
//! GET /search/docs?query=filter&hitsPerPage=20&page=0
//! ```
//!
//! without exposing the full trusted `Execute` / `/v1/execute` surface and
//! without requiring every application to build a bespoke backend-for-frontend.
//!
//! This module remains inside the `searchkerneld` crate. It is not a separate
//! crate, not a reusable search engine, and not a second implementation of
//! Search Kernel semantics.
//!
//! # Architectural boundary
//!
//! The intended request path is:
//!
//! ```text
//! public HTTP request
//!         ↓
//! searchkerneld listener / admission control
//!         ↓
//! search_endpoints
//!     route selection
//!     endpoint-policy lookup
//!     public-input decoding
//!     type/bounds validation
//!     prepared-request binding
//!         ↓
//! canonical Search Kernel RequestEnvelope bytes
//!         ↓
//! existing searchkernel-dispatch::Dispatcher
//!         ↓
//! *_to_ir()
//!         ↓
//! backend / Vespa
//! ```
//!
//! The response path is:
//!
//! ```text
//! canonical Search Kernel ResponseEnvelope bytes
//!         ↓
//! search_endpoints
//!     allowed result projection
//!     endpoint JSON representation
//!         ↓
//! public HTTP response
//! ```
//!
//! The module may own **binding and exposure policy**. It must not own search
//! meaning.
//!
//! In particular, this module must never independently implement:
//!
//! - text analysis or linguistic behavior;
//! - query-tree semantics;
//! - filter parsing or filter semantics;
//! - schema field legality;
//! - ranking semantics;
//! - grouping/facet semantics;
//! - Search Kernel defaults;
//! - result meaning;
//! - backend lowering;
//! - a second dispatcher;
//! - docs-specific search behavior;
//! - Algolia search semantics.
//!
//! All endpoint requests must eventually enter the same canonical
//! `searchkernel-dispatch` -> `*_to_ir()` path used by the trusted portable
//! API.
//!
//! # Why this is a module in `searchkerneld`
//!
//! Search Endpoints are a daemon serving concern:
//!
//! - they expose HTTP routes;
//! - they constrain an otherwise powerful daemon;
//! - they bind untrusted public inputs;
//! - they apply request-cost and response-exposure policy;
//! - they share daemon admission/execution limits;
//! - they are deployed with a particular `searchkerneld` instance/index.
//!
//! They are therefore not part of the embedded Rust API and should not become a
//! core semantic crate. Keeping the implementation as an internal
//! `searchkerneld` module also makes it harder for endpoint-only concerns to
//! leak into `searchkernel-ir` or the canonical protocol.
//!
//! # Search Endpoint model
//!
//! A Search Endpoint should be treated conceptually as a **prepared Search
//! Kernel request with explicitly exposed variable slots**.
//!
//! An endpoint definition is expected to describe at least:
//!
//! ```text
//! identity
//!     endpoint name
//!     public path
//!
//! target
//!     fixed index / daemon target
//!
//! prepared request
//!     fixed search/query shape
//!     fixed ranking profile where applicable
//!     fixed search fields / weights where applicable
//!     fixed filters or filter template where applicable
//!     fixed grouping/facets where applicable
//!     fixed result projection
//!
//! exposed inputs
//!     public parameter name
//!     target binding slot
//!     type
//!     required / optional
//!     default when explicitly defined
//!     bounds / enum allowlist
//!
//! serving policy
//!     maximum result count
//!     maximum page/offset
//!     timeout ceiling
//!     request-size ceiling
//!     batch-size ceiling
//!     allowed origins / CORS policy
//! ```
//!
//! The configuration format is deliberately **not frozen by this stub**. TOML,
//! JSON, YAML, generated configuration, or a typed Rust loader may be evaluated
//! separately. The important contract is the model and the lowering path, not
//! the serialization syntax of daemon configuration.
//!
//! Endpoint configuration belongs to serving/deployment configuration, not to
//! the index schema. One index may legitimately expose multiple endpoint
//! surfaces, for example:
//!
//! ```text
//! /search
//! /autocomplete
//! /category-search
//! ```
//!
//! with different fixed rankings, projections, limits, and exposed controls.
//!
//! # Parameter binding
//!
//! Public inputs are data. They are not arbitrary Search Kernel source text.
//!
//! Prefer bindings such as:
//!
//! ```text
//! query (InstantSearch) / q (general endpoint alias)
//!     -> search text
//!
//! hitsPerPage (InstantSearch) / limit (general endpoint alias)
//!     -> request limit, bounded to <= endpoint maximum
//!
//! page (InstantSearch, zero-based)
//!     -> checked offset = page * effective hitsPerPage
//!
//! language
//!     -> named filter binding :language
//!
//! sort=relevance|newest
//!     -> one of a fixed set of predeclared sort expressions
//! ```
//!
//! over exposing unrestricted controls such as:
//!
//! ```text
//! filter=<arbitrary expression>
//! yql=<arbitrary source>
//! ranking=<arbitrary profile>
//! sort_field=<arbitrary field>
//! ```
//!
//! The endpoint definition should explicitly declare every variable the public
//! caller is allowed to influence. Unknown public parameters should fail closed
//! rather than being silently ignored.
//!
//! ## Presence semantics
//!
//! Binding must preserve absence versus explicit values. In particular:
//!
//! - absent is not the same as an empty string;
//! - absent is not the same as an empty list;
//! - absent is not the same as zero;
//! - absent is not the same as false;
//! - endpoint defaults apply only where the endpoint definition explicitly
//!   defines one;
//! - a public parameter must not accidentally trigger a portable default merely
//!   because an HTTP parser supplied a host-language zero value.
//!
//! The one deliberate query-binding normalization is explicit: omitted,
//! empty, and whitespace-only `query` / `q` values represent the public browse state and
//! bind to canonical `MatchAllQuery`. Canonical Search Kernel semantics reject
//! empty/whitespace text queries, so the endpoint must never manufacture a
//! known-invalid `SearchKernelTextQuery("")`.
//!
//! This matters because the canonical Search Kernel API has presence-sensitive
//! fields.
//!
//! ## Numeric inputs
//!
//! Public endpoint numeric parsing must preserve Search Kernel's portable
//! numeric rules. Do not round or silently coerce:
//!
//! - signed and unsigned integer bounds must be checked exactly;
//! - fractional values must not be accepted for integer bindings;
//! - non-finite floats must be rejected;
//! - any decimal-string representation used to preserve 64-bit values must be
//!   converted losslessly;
//! - overflow in page/offset/limit arithmetic must be rejected before request
//!   construction.
//!
//! ## Repeated parameters
//!
//! The eventual endpoint contract must define repeated-parameter behavior
//! explicitly. It must not accidentally depend on whichever query-string parser
//! happens to be used. Each exposed input should declare whether it is scalar or
//! repeated, and duplicate scalar parameters should normally be rejected.
//!
//! # Prepared request construction
//!
//! The endpoint binder must construct the canonical portable request; it must
//! not lower directly to Search Kernel IR.
//!
//! The intended ownership is:
//!
//! ```text
//! endpoint policy + public inputs
//!         ↓
//! canonical protocol request builder
//!         ↓
//! RequestEnvelope FlatBuffer bytes
//!         ↓
//! Dispatcher::dispatch(...)
//! ```
//!
//! Do not call backend APIs directly from this module.
//!
//! Do not create an `EndpointIr` that bypasses the portable API merely to make
//! implementation convenient.
//!
//! If the canonical portable request cannot express something required by a
//! Search Endpoint, that is a product-contract question to resolve in the
//! canonical API rather than an excuse for an alternate semantic path.
//!
//! # GET and POST
//!
//! A simple endpoint should support an ergonomic GET form where the exposed
//! parameter set is naturally query-string shaped:
//!
//! ```text
//! GET /search/docs?query=filter&hitsPerPage=10&page=0
//! ```
//!
//! A POST form is likely necessary for:
//!
//! - batched requests;
//! - larger structured public inputs;
//! - InstantSearch multi-query behavior;
//! - avoiding impractical URL lengths.
//!
//! GET and POST must bind through the same endpoint definition and produce
//! equivalent canonical requests for equivalent inputs. They must not become two
//! semantic implementations.
//!
//! # Batching
//!
//! InstantSearch-compatible UIs may issue multiple logical searches for one UI
//! interaction. Search Endpoints should therefore be designed with bounded
//! batching in mind from the start.
//!
//! Conceptually:
//!
//! ```text
//! POST /search/docs
//! {
//!   "requests": [
//!     { ... },
//!     { ... }
//!   ]
//! }
//! ```
//!
//! Batching policy must define:
//!
//! - maximum requests per batch;
//! - per-request validation;
//! - aggregate request-size limits;
//! - aggregate execution/admission behavior;
//! - response ordering;
//! - whether one invalid request rejects the whole batch or produces a
//!   per-request error.
//!
//! The chosen error behavior must be explicit and covered by compatibility
//! tests. Never allow batching to bypass daemon admission/execution bounds.
//!
//! # Public result representation
//!
//! Search Endpoint JSON is intentionally an **application-facing projection**,
//! not another canonical Search Kernel domain contract.
//!
//! The canonical portable response remains the authority underneath it.
//!
//! A public response will likely need a stable small shape containing concepts
//! such as:
//!
//! ```text
//! totalHits
//! offset / page
//! limit / hitsPerPage
//! hits[]
//!     id
//!     score where exposed
//!     projected fields
//!     highlights/snippets where configured
//! facets/grouping projections where configured
//! ```
//!
//! Endpoint configuration should bound which stored fields, highlights,
//! grouping/facet outputs, and metadata are publicly exposed.
//!
//! Do not expose backend/internal fields merely because the canonical response
//! contains them.
//!
//! Response shaping must be deterministic and must preserve the meaning of the
//! canonical response. It may rename/project for ergonomic JSON, but it must not
//! recalculate ranking, facet counts, total-hit semantics, or other search
//! results.
//!
//! # Public versus trusted daemon surfaces
//!
//! Search Endpoints are intended to solve a security/product problem that the
//! generic `Execute` surface does not solve by itself.
//!
//! The eventual daemon configuration should support a topology such as:
//!
//! ```text
//! private / trusted listener
//!     gRPC Execute(...)
//!     gRPC-Web Execute(...)
//!     POST /v1/execute
//!
//! public serving listener
//!     named Search Endpoints only
//! ```
//!
//! Do not assume operation rejection inside a public generic `Execute` handler
//! is equivalent to not exposing the generic handler at all.
//!
//! The public listener should be able to omit trusted mutation/admin routes
//! entirely.
//!
//! Search Endpoint handlers must still share the daemon's global admission and
//! bounded execution controls unless later architecture explicitly introduces
//! separately budgeted pools.
//!
//! # Mandatory public-security release gate
//!
//! Public Search Endpoints are anonymous internet-facing ingress. They must be
//! designed as a hostile-input boundary, not merely as ergonomic HTTP routing.
//!
//! A public listener MUST remain disabled by default and MUST NOT be enabled in
//! a supported deployment until the security acceptance suite described below
//! has executable verification-DAG ownership.
//!
//! ## Capability isolation
//!
//! The public listener must expose only named Search Endpoint routes. It must
//! not mount or fall through to:
//!
//! - gRPC `Execute`;
//! - gRPC-Web `Execute`;
//! - `/v1/execute`;
//! - writes or mutation operations;
//! - schema/index administration;
//! - arbitrary canonical `RequestEnvelope` execution.
//!
//! Public callers may influence only parameters explicitly declared by the
//! endpoint. Raw YQL, arbitrary filter source, arbitrary field names, arbitrary
//! ranking profiles, arbitrary grouping plans, and similarly open-ended
//! controls are forbidden unless a future endpoint capability models them with
//! its own bounded allowlist.
//!
//! Public result projection must also be allowlisted. Backend/internal fields,
//! feature maps, traces, stack traces, or other metadata must never become
//! public merely because they are present in the canonical response.
//!
//! ## Bound before expensive work
//!
//! Cheap limits must run before percent-decoding, large allocation, canonical
//! FlatBuffer construction, dispatch, or backend execution wherever possible.
//! At minimum the final listener needs explicit bounds for:
//!
//! - raw request-target / URI bytes;
//! - raw query-string bytes;
//! - HTTP header bytes/count to the extent supported by the HTTP stack;
//! - POST body bytes;
//! - number of query parameters;
//! - decoded query bytes;
//! - batch request count and aggregate batch bytes;
//! - result limit;
//! - pagination depth/offset;
//! - response bytes.
//!
//! Future facets, grouping, refinements, vector search, and nearest-neighbor
//! controls require explicit cardinality/complexity ceilings before they can be
//! exposed publicly.
//!
//! All size/cost arithmetic must be overflow-checked. Oversized requests fail
//! before canonical request construction.
//!
//! ## Parser ambiguity / differential defense
//!
//! The public parser must have one authoritative interpretation:
//!
//! - reject duplicate scalar parameters;
//! - reject unknown parameters;
//! - perform percent-decoding exactly once;
//! - reject malformed or truncated percent escapes;
//! - reject invalid UTF-8;
//! - define `+` handling explicitly;
//! - reject signs, fractions, whitespace, and overflow for unsigned integer
//!   parameters;
//! - test percent-encoded parameter names and delimiter spellings;
//! - reject empty parameter segments if the contract does not allow them;
//! - never normalize attacker-controlled path spellings into trusted/admin
//!   routes.
//!
//! Equivalent GET and POST inputs must bind to equivalent canonical requests.
//!
//! ## Computational denial-of-service defense
//!
//! Public admission must be bounded and fail fast. There must be no unbounded
//! queue of public work.
//!
//! Public execution must have an explicit concurrency budget, either isolated
//! from or deliberately budgeted against trusted daemon traffic.
//!
//! A request deadline must bound public serving latency, but an HTTP timeout
//! must not be treated as backend cancellation unless native execution is
//! actually cancelled. If backend/native work can continue after the client
//! gives up, complexity limits plus concurrency/admission bounds must still
//! protect the process.
//!
//! Batched requests must consume budget proportional to their fan-out rather
//! than obtaining one admission token for arbitrary work.
//!
//! ## Network-abuse posture
//!
//! CORS is compatibility policy, not authentication and not a security
//! boundary. Wildcard origins must not be the secure default.
//!
//! Direct public deployment needs a rate-limit story. Rate limiting may be
//! built into `searchkerneld` or enforced by a mandatory documented reverse
//! proxy/deployment boundary, but public deployment must not silently omit it.
//!
//! TLS termination must be supported/documented. Slow clients must be bounded
//! with header/body read deadlines and connection/resource limits so slowloris
//! traffic cannot pin admission/execution capacity.
//!
//! ## Errors and privacy
//!
//! Public errors must use a small stable error domain. Never return arbitrary
//! native/backend error text, stack traces, filesystem paths, schema internals,
//! panic messages, or implementation details.
//!
//! Raw query/filter values must not be logged by default. Operational telemetry
//! may record endpoint identity, status/error category, latency, result count,
//! batch size, admission rejection, and similar non-content metadata.
//!
//! A panic reachable from public input is a security defect.
//!
//! ## Hostile verification
//!
//! Before a public listener is enabled, the focused Search Endpoint verifier
//! must include:
//!
//! - a dedicated test target whose absence makes the verifier fail rather than
//!   succeed with zero matching tests;
//! - exact public-input -> `Dispatcher::dispatch` -> `*_to_ir()` ->
//!   RecordingBackend proof;
//! - proof invalid public input never reaches the backend;
//! - property/fuzz testing of parsing and binding;
//! - malformed percent-escape, overlong encoding, invalid UTF-8, duplicate and
//!   encoded parameter-name, delimiter, numeric overflow, and exact-boundary
//!   corpora;
//! - admission/concurrency exhaustion tests;
//! - deadline/cancellation tests;
//! - batch aggregate-cost tests;
//! - response-field leak tests;
//! - public-listener isolation tests proving trusted/admin endpoints are
//!   unreachable;
//! - regression tests for every security finding before its fix.
//!
//! Endpoint policy must make dangerous or expensive behavior impossible by
//! construction rather than relying on caller discipline.
//!
//! # Errors
//!
//! The public error domain should remain small and intentional.
//!
//! Likely categories include:
//!
//! - endpoint not found;
//! - method not allowed;
//! - malformed public input;
//! - unknown parameter;
//! - missing required parameter;
//! - invalid enum/value/type;
//! - request exceeds endpoint policy;
//! - request rejected by admission limits;
//! - canonical Search Kernel semantic error;
//! - endpoint target unavailable/closed;
//! - internal response-projection failure.
//!
//! Canonical Search Kernel semantic failures must be mapped, not reinterpreted.
//! Error mapping must be deterministic and must not leak internal/native error
//! text that was never intended as public API.
//!
//! # InstantSearch integration boundary
//!
//! The InstantSearch adapter implementation belongs in
//! `integrations/instantsearch`, but compatibility is a hard acceptance
//! requirement for this module's public request contract. A Search Endpoint
//! change is not considered complete merely because `q/limit/offset` works;
//! the dedicated InstantSearch verifier must prove the equivalent
//! `query/hitsPerPage/page` path, including browse, Unicode, pagination
//! arithmetic, ambiguity rejection, policy bounds, and canonical dispatcher/IR
//! convergence.
//!
//! The division is:
//!
//! ```text
//! InstantSearch request/result contract
//!         ↓
//! integrations/instantsearch
//!         ↓
//! Search Endpoint public request/result contract
//!         ↓
//! searchkerneld::search_endpoints
//!         ↓
//! canonical Search Kernel protocol
//! ```
//!
//! This module should expose a clean general-purpose endpoint contract that is
//! useful without InstantSearch.
//!
//! The InstantSearch adapter may translate:
//!
//! - query text;
//! - page / hits-per-page;
//! - configured refinements;
//! - configured facet requests;
//! - configured sort choices;
//! - highlight/snippet result structures;
//! - batched searches.
//!
//! Do **not** make `searchkerneld` emulate Algolia's complete HTTP API. Widget
//! compatibility should be implemented and tested in the integration adapter.
//!
//! # Documentation dogfood
//!
//! Search Kernel's own GitHub-Pages documentation is the intended first
//! end-to-end acceptance application:
//!
//! ```text
//! GitHub Pages
//!     ↓
//! InstantSearch.js
//!     ↓
//! integrations/instantsearch
//!     ↓
//! public docs Search Endpoint
//!     ↓
//! searchkerneld
//!     ↓
//! canonical Search Kernel path
//!     ↓
//! docs index
//! ```
//!
//! There must be no docs-specific search daemon or bespoke backend service.
//!
//! Friction discovered while building that application should be treated as
//! evidence about the general Search Endpoint product surface.
//!
//! # Configuration validation
//!
//! Endpoint definitions should be validated completely at daemon startup where
//! possible. Invalid endpoint configuration should fail startup rather than
//! defer obvious configuration errors until the first public request.
//!
//! Startup validation should eventually cover at least:
//!
//! - duplicate endpoint names/routes;
//! - unsupported parameter types;
//! - duplicate public parameter names;
//! - bindings to nonexistent prepared-request slots;
//! - invalid fixed sort/ranking/facet declarations;
//! - invalid result projections;
//! - contradictory required/default declarations;
//! - impossible numeric bounds;
//! - unsafe/unbounded controls that violate required server policy;
//! - reserved route collisions such as `/v1/execute`.
//!
//! Runtime request validation should then deal only with caller-supplied data and
//! dynamic target/backend state.
//!
//! # Concurrency and lifecycle
//!
//! Endpoint definitions should be immutable after server startup for the first
//! implementation unless live reload is deliberately designed later.
//!
//! Immutability simplifies:
//!
//! - request races;
//! - route lookup;
//! - prepared-template sharing;
//! - ownership/lifetime;
//! - exact behavior under concurrent requests.
//!
//! If live configuration reload is ever added, it must define atomic visibility
//! and in-flight request behavior explicitly.
//!
//! Search Endpoint execution must honor the same close/shutdown semantics as the
//! existing daemon transport. Requests must not bypass target-closed checks by
//! retaining an alternate backend handle.
//!
//! # Observability
//!
//! Keep operational metadata separate from search semantics.
//!
//! Useful endpoint-level observations may include:
//!
//! - endpoint name;
//! - request duration;
//! - response status/error category;
//! - result count;
//! - admission rejection;
//! - batch size;
//! - timeout/cancellation.
//!
//! Do not log raw public query text or filter values by default. Documentation or
//! application deployments may choose explicit privacy-aware query analytics
//! separately.
//!
//! # Verification and false-green risks
//!
//! Search Endpoints are a new semantic ingress/public serving surface. Before
//! behavioral implementation lands, the authoritative verification DAG must be
//! updated with explicit ownership, selector paths, a canonical test command,
//! dependency closure, runner class where relevant, and full-suite reachability.
//!
//! Tests must go beyond happy-path HTTP examples. In particular, try to falsify
//! the implementation with:
//!
//! - malformed percent encoding / malformed JSON;
//! - unknown parameters;
//! - duplicate scalar parameters;
//! - absent versus explicit empty/zero/false;
//! - invalid UTF-8 boundaries as applicable;
//! - integer overflow and fractional-to-integer coercion;
//! - non-finite float representations;
//! - maximum and just-over-maximum body/query/batch sizes;
//! - page/offset/limit arithmetic overflow;
//! - unsupported sort/ranking/facet choices;
//! - attempts to bind fields not exposed by the endpoint;
//! - attempts to smuggle raw filter/YQL/query controls;
//! - response projections containing missing/unsupported fields;
//! - simultaneous requests at admission/execution limits;
//! - client disconnect/cancellation;
//! - daemon close while endpoint work is queued or executing;
//! - malformed canonical responses during projection tests;
//! - GET/POST equivalence;
//! - deterministic batch response ordering;
//! - CORS behavior;
//! - public listener accidentally exposing `/v1/execute` or gRPC-Web Execute;
//! - endpoint route collisions;
//! - configuration reload/reopen behavior if reload is ever introduced.
//!
//! Exact-head E2E verification should prove that at least one endpoint request
//! reaches the real canonical dispatcher/backend path rather than a mocked
//! shortcut.
//!
//! # Suggested implementation sequence
//!
//! Keep implementation in bounded review units:
//!
//! ```text
//! SE0  endpoint definition/configuration model + startup validation
//! SE1  single-request binder -> canonical SearchRequest/RequestEnvelope,
//!      including the InstantSearch core request profile
//! SE2  public listener routing + strict public/trusted surface separation
//! SE3  ergonomic projected JSON response + error model
//! SE4  batching + aggregate limits
//! SE5  facets/refinements/highlighting/sort controls required by UI adapters
//! SE6  InstantSearch compatibility acceptance through integrations/instantsearch
//! SE7  docs-search end-to-end dogfood/relevance acceptance
//! ```
//!
//! The sequencing is not permission to combine these into one oversized PR.
//!
//! # Initial non-goals
//!
//! The first implementation should not include:
//!
//! - a new crate;
//! - an alternate Search Kernel protocol;
//! - a second JSON-to-IR path;
//! - direct backend invocation;
//! - arbitrary public access to every SearchRequest field;
//! - Algolia HTTP wire compatibility;
//! - endpoint-specific search semantics;
//! - docs-specific code;
//! - mutation endpoints;
//! - schema/index administration;
//! - live endpoint reconfiguration;
//! - authentication/authorization framework beyond the serving-policy boundary
//!   required for safe public search;
//! - analytics/telemetry product semantics.
//!
//! # First-slice rule
//!
//! Keep this module's executable surface smaller than the eventual public HTTP
//! contract. The current typed binder exists to prove policy validation,
//! presence preservation, and canonical request construction before routing
//! and response-shaping are introduced. Do not grow it into a parallel search
//! API or bypass the canonical dispatcher path when later slices are added.

#![cfg_attr(not(feature = "test-support"), allow(dead_code))]

use std::{sync::Arc, time::Duration};

use searchkernel_ir::validate_index_target;
use searchkernel_protocol::{
    protocol::v1::{self as wire, PROTOCOL_VERSION},
    wire::v1 as api,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError},
    task::JoinHandle,
};

/// Independent fail-fast concurrency budgets for anonymous Search Endpoint work.
///
/// Public traffic must never queue indefinitely for either admission or backend
/// execution. The trusted daemon transport has its own concurrency controls;
/// these limits are deliberately separate so anonymous callers cannot consume
/// every trusted execution slot merely by filling the public queue.
#[derive(Clone, Debug)]
pub struct SearchEndpointConcurrency {
    admission: Arc<Semaphore>,
    execution: Arc<Semaphore>,
}

impl SearchEndpointConcurrency {
    pub fn new(
        admission_limit: usize,
        execution_limit: usize,
    ) -> Result<Self, SearchEndpointConcurrencyError> {
        if admission_limit == 0 {
            return Err(SearchEndpointConcurrencyError::InvalidAdmissionLimit);
        }
        if admission_limit > Semaphore::MAX_PERMITS {
            return Err(SearchEndpointConcurrencyError::AdmissionLimitTooLarge);
        }
        if execution_limit == 0 {
            return Err(SearchEndpointConcurrencyError::InvalidExecutionLimit);
        }
        if execution_limit > Semaphore::MAX_PERMITS {
            return Err(SearchEndpointConcurrencyError::ExecutionLimitTooLarge);
        }
        Ok(Self {
            admission: Arc::new(Semaphore::new(admission_limit)),
            execution: Arc::new(Semaphore::new(execution_limit)),
        })
    }

    /// Attempts public admission without waiting.
    pub fn try_admit(
        &self,
    ) -> Result<SearchEndpointAdmissionPermit, SearchEndpointConcurrencyError> {
        self.admission
            .clone()
            .try_acquire_owned()
            .map(|permit| SearchEndpointAdmissionPermit { _permit: permit })
            .map_err(|error| match error {
                TryAcquireError::NoPermits => SearchEndpointConcurrencyError::AdmissionOverloaded,
                TryAcquireError::Closed => SearchEndpointConcurrencyError::Unavailable,
            })
    }

    /// Starts one blocking backend operation only when execution capacity is
    /// available immediately.
    ///
    /// The execution permit is moved into the blocking task itself. If the
    /// caller drops the returned JoinHandle because an HTTP deadline expires or
    /// the client disconnects, already-started backend work retains its permit
    /// until it actually finishes. This prevents timeout/cancellation from
    /// creating phantom capacity while native work is still consuming CPU.
    pub fn try_spawn_blocking<T, F>(
        &self,
        operation: F,
    ) -> Result<JoinHandle<T>, SearchEndpointConcurrencyError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let permit = self
            .execution
            .clone()
            .try_acquire_owned()
            .map_err(|error| match error {
                TryAcquireError::NoPermits => SearchEndpointConcurrencyError::ExecutionOverloaded,
                TryAcquireError::Closed => SearchEndpointConcurrencyError::Unavailable,
            })?;
        Ok(tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation()
        }))
    }
}

/// Waits for one already-started blocking Search Endpoint operation.
///
/// Timing out stops waiting for the result; it does not claim to cancel native
/// work. The blocking task is detached and continues holding its execution
/// permit until the operation actually exits.
pub async fn await_blocking_with_timeout<T>(
    handle: JoinHandle<T>,
    timeout: Duration,
) -> Result<T, SearchEndpointExecutionWaitError>
where
    T: Send + 'static,
{
    let mut handle = handle;
    match tokio::time::timeout(timeout, &mut handle).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(SearchEndpointExecutionWaitError::TaskFailed),
        Err(_) => {
            drop(handle);
            Err(SearchEndpointExecutionWaitError::TimedOut)
        }
    }
}

/// Stable wait outcomes for public blocking execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEndpointExecutionWaitError {
    TimedOut,
    TaskFailed,
}

impl std::fmt::Display for SearchEndpointExecutionWaitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TimedOut => "Search Endpoint execution timed out",
            Self::TaskFailed => "Search Endpoint execution task failed",
        })
    }
}

impl std::error::Error for SearchEndpointExecutionWaitError {}

/// Held for the lifetime of one admitted public request.
#[derive(Debug)]
pub struct SearchEndpointAdmissionPermit {
    _permit: OwnedSemaphorePermit,
}

/// Stable fail-fast capacity errors for the public Search Endpoint edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEndpointConcurrencyError {
    InvalidAdmissionLimit,
    AdmissionLimitTooLarge,
    InvalidExecutionLimit,
    ExecutionLimitTooLarge,
    AdmissionOverloaded,
    ExecutionOverloaded,
    Unavailable,
}

impl std::fmt::Display for SearchEndpointConcurrencyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidAdmissionLimit => {
                "Search Endpoint admission limit must be greater than zero"
            }
            Self::AdmissionLimitTooLarge => "Search Endpoint admission limit is too large",
            Self::InvalidExecutionLimit => {
                "Search Endpoint execution limit must be greater than zero"
            }
            Self::ExecutionLimitTooLarge => "Search Endpoint execution limit is too large",
            Self::AdmissionOverloaded => "Search Endpoint admission capacity is exhausted",
            Self::ExecutionOverloaded => "Search Endpoint execution capacity is exhausted",
            Self::Unavailable => "Search Endpoint capacity is unavailable",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SearchEndpointConcurrencyError {}

/// Request-cost bounds owned by one public Search Endpoint.
///
/// These are serving-policy constraints, not Search Kernel query semantics.
/// A missing default remains missing in the canonical request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchEndpointPolicy {
    max_query_string_bytes: usize,
    max_parameters: usize,
    max_query_bytes: usize,
    default_limit: Option<u32>,
    max_limit: u32,
    max_offset: u32,
}

impl SearchEndpointPolicy {
    /// Creates explicit serving-policy bounds for a Search Endpoint.
    ///
    /// No result-count default is invented here. Call
    /// [`Self::with_default_limit`] when an endpoint deliberately wants one.
    pub const fn new(
        max_query_string_bytes: usize,
        max_parameters: usize,
        max_query_bytes: usize,
        max_limit: u32,
        max_offset: u32,
    ) -> Self {
        Self {
            max_query_string_bytes,
            max_parameters,
            max_query_bytes,
            default_limit: None,
            max_limit,
            max_offset,
        }
    }

    /// Supplies an explicit endpoint-level default result limit.
    pub const fn with_default_limit(mut self, default_limit: u32) -> Self {
        self.default_limit = Some(default_limit);
        self
    }

    fn validate(&self) -> Result<(), SearchEndpointBindError> {
        if let Some(default_limit) = self.default_limit {
            if default_limit > self.max_limit {
                return Err(SearchEndpointBindError::InvalidPolicy(
                    "default_limit exceeds max_limit",
                ));
            }
        } else if self.max_limit < searchkernel_ir::DEFAULT_SEARCH_LIMIT {
            return Err(SearchEndpointBindError::InvalidPolicy(
                "max_limit below canonical default requires explicit default_limit",
            ));
        }
        Ok(())
    }
}

/// One named public Search Endpoint.
///
/// The public route is derived as `/search/{name}`. Names are validated when
/// the endpoint registry is built so route matching never depends on decoding
/// arbitrary path fragments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchEndpointConfig {
    name: String,
    target: String,
    policy: SearchEndpointPolicy,
}

impl SearchEndpointConfig {
    /// Creates a named Search Endpoint bound to one canonical logical index target.
    pub fn new(
        name: impl Into<String>,
        target: impl Into<String>,
        policy: SearchEndpointPolicy,
    ) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            policy,
        }
    }

    /// Returns the stable endpoint name used in `/search/{name}`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the canonical logical index target for this endpoint.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the endpoint serving policy.
    pub fn policy(&self) -> &SearchEndpointPolicy {
        &self.policy
    }

    /// Returns the public route for this endpoint.
    pub fn route(&self) -> String {
        format!("/search/{}", self.name)
    }
}

#[derive(Clone, Debug)]
pub struct SearchEndpointRouter {
    endpoints: std::collections::BTreeMap<String, SearchEndpointConfig>,
}

impl SearchEndpointRouter {
    pub fn new(
        endpoints: impl IntoIterator<Item = SearchEndpointConfig>,
    ) -> Result<Self, SearchEndpointRouteError> {
        let mut by_route = std::collections::BTreeMap::new();
        for endpoint in endpoints {
            validate_endpoint_name(endpoint.name())?;
            validate_index_target(endpoint.target()).map_err(|error| {
                SearchEndpointRouteError::InvalidIndexTarget {
                    target: endpoint.target().to_owned(),
                    reason: error.to_string(),
                }
            })?;
            endpoint
                .policy
                .validate()
                .map_err(SearchEndpointRouteError::Bind)?;
            let route = endpoint.route();
            if by_route.insert(route.clone(), endpoint).is_some() {
                return Err(SearchEndpointRouteError::DuplicateRoute(route));
            }
        }
        Ok(Self {
            endpoints: by_route,
        })
    }

    pub fn bind_get(
        &self,
        path: &str,
        raw_query: Option<&str>,
    ) -> Result<Vec<u8>, SearchEndpointRouteError> {
        let endpoint = self
            .endpoints
            .get(path)
            .ok_or(SearchEndpointRouteError::UnknownRoute)?;
        let request = decode_get_query(endpoint.policy(), raw_query)?;
        bind_search_request(endpoint.target(), endpoint.policy(), request)
            .map_err(SearchEndpointRouteError::Bind)
    }
}

fn validate_endpoint_name(name: &str) -> Result<(), SearchEndpointRouteError> {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return Err(SearchEndpointRouteError::InvalidEndpointName(
            name.to_owned(),
        ));
    };
    if !first.is_ascii_alphanumeric()
        || bytes.any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    {
        return Err(SearchEndpointRouteError::InvalidEndpointName(
            name.to_owned(),
        ));
    }
    Ok(())
}

/// Fail-closed route/query decoding failures for the public Search Endpoint edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchEndpointRouteError {
    InvalidEndpointName(String),
    InvalidIndexTarget {
        target: String,
        reason: String,
    },
    DuplicateRoute(String),
    UnknownRoute,
    QueryStringTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    TooManyParameters {
        actual: usize,
        max: usize,
    },
    MalformedQuery(&'static str),
    InvalidUtf8,
    InvalidQueryText,
    UnknownParameter,
    DuplicateParameter(&'static str),
    InvalidUnsignedInteger {
        parameter: &'static str,
    },
    ConflictingPaginationControls,
    PageRequiresHitsPerPage,
    PaginationOverflow,
    Bind(SearchEndpointBindError),
}

impl std::fmt::Display for SearchEndpointRouteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEndpointName(name) => {
                write!(formatter, "invalid Search Endpoint name {name:?}")
            }
            Self::InvalidIndexTarget { target, reason } => {
                write!(
                    formatter,
                    "invalid Search Endpoint index target {target:?}: {reason}"
                )
            }
            Self::DuplicateRoute(route) => {
                write!(formatter, "duplicate Search Endpoint route {route:?}")
            }
            Self::UnknownRoute => write!(formatter, "unknown Search Endpoint route"),
            Self::QueryStringTooLarge {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "Search Endpoint query string is {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::TooManyParameters { actual, max } => write!(
                formatter,
                "Search Endpoint request has {actual} parameters; maximum is {max}"
            ),
            Self::MalformedQuery(message) => write!(formatter, "malformed query string: {message}"),
            Self::InvalidUtf8 => write!(formatter, "query parameter is not valid UTF-8"),
            Self::InvalidQueryText => {
                write!(
                    formatter,
                    "query text contains unsupported control characters"
                )
            }
            Self::UnknownParameter => write!(formatter, "unknown query parameter"),
            Self::DuplicateParameter(_) => {
                write!(formatter, "duplicate scalar query parameter")
            }
            Self::InvalidUnsignedInteger { parameter } => write!(
                formatter,
                "query parameter {parameter:?} must be an unsigned decimal integer"
            ),
            Self::ConflictingPaginationControls => write!(
                formatter,
                "InstantSearch page cannot be combined with explicit offset"
            ),
            Self::PageRequiresHitsPerPage => write!(
                formatter,
                "InstantSearch page requires hitsPerPage or an endpoint default"
            ),
            Self::PaginationOverflow => {
                write!(formatter, "InstantSearch pagination arithmetic overflow")
            }
            Self::Bind(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SearchEndpointRouteError {}

fn decode_get_query(
    policy: &SearchEndpointPolicy,
    raw_query: Option<&str>,
) -> Result<SearchEndpointRequest, SearchEndpointRouteError> {
    let mut query = None;
    let mut limit = None;
    let mut offset = None;
    let mut page = None;

    let Some(raw_query) = raw_query.filter(|query| !query.is_empty()) else {
        return Ok(SearchEndpointRequest::match_all());
    };

    if raw_query.len() > policy.max_query_string_bytes {
        return Err(SearchEndpointRouteError::QueryStringTooLarge {
            actual_bytes: raw_query.len(),
            max_bytes: policy.max_query_string_bytes,
        });
    }

    let mut parameter_count = 0usize;
    for pair in raw_query.split('&') {
        parameter_count =
            parameter_count
                .checked_add(1)
                .ok_or(SearchEndpointRouteError::MalformedQuery(
                    "parameter count overflow",
                ))?;
        if parameter_count > policy.max_parameters {
            return Err(SearchEndpointRouteError::TooManyParameters {
                actual: parameter_count,
                max: policy.max_parameters,
            });
        }

        if pair.is_empty() {
            return Err(SearchEndpointRouteError::MalformedQuery(
                "empty parameter segment",
            ));
        }

        let (raw_name, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let parameter = match raw_name {
            // `query`, `hitsPerPage`, and zero-based `page` are the
            // InstantSearch.js SearchClient compatibility vocabulary. The short
            // names remain the general Search Endpoint aliases.
            "q" | "query" => "query",
            "limit" | "hitsPerPage" => "limit",
            "offset" => "offset",
            "page" => "page",
            _ => return Err(SearchEndpointRouteError::UnknownParameter),
        };
        let value = strict_query_component(raw_value)?;

        match parameter {
            "query" => {
                if query.replace(value).is_some() {
                    return Err(SearchEndpointRouteError::DuplicateParameter("query"));
                }
            }
            "limit" => {
                if limit.is_some() {
                    return Err(SearchEndpointRouteError::DuplicateParameter("hitsPerPage"));
                }
                limit = Some(parse_u32_parameter(
                    if raw_name == "hitsPerPage" {
                        "hitsPerPage"
                    } else {
                        "limit"
                    },
                    &value,
                )?);
            }
            "offset" => {
                if offset.is_some() {
                    return Err(SearchEndpointRouteError::DuplicateParameter("offset"));
                }
                offset = Some(parse_u32_parameter("offset", &value)?);
            }
            "page" => {
                if page.is_some() {
                    return Err(SearchEndpointRouteError::DuplicateParameter("page"));
                }
                page = Some(parse_u32_parameter("page", &value)?);
            }
            _ => unreachable!("parameter allowlist is exhaustive"),
        }
    }

    if page.is_some() && offset.is_some() {
        return Err(SearchEndpointRouteError::ConflictingPaginationControls);
    }

    if let Some(page) = page {
        let hits_per_page = limit.or(policy.default_limit);
        offset = Some(match hits_per_page {
            Some(hits_per_page) => page
                .checked_mul(hits_per_page)
                .ok_or(SearchEndpointRouteError::PaginationOverflow)?,
            None if page == 0 => 0,
            None => return Err(SearchEndpointRouteError::PageRequiresHitsPerPage),
        });
    }

    let mut request = match query {
        Some(query) if query.chars().any(char::is_control) => {
            return Err(SearchEndpointRouteError::InvalidQueryText);
        }
        Some(query) if !query.trim().is_empty() => SearchEndpointRequest::text(query),
        Some(_) | None => SearchEndpointRequest::match_all(),
    };
    request.limit = limit;
    request.offset = offset;
    Ok(request)
}

fn strict_query_component(raw: &str) -> Result<String, SearchEndpointRouteError> {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(SearchEndpointRouteError::MalformedQuery(
                        "truncated percent escape",
                    ));
                }
                let high = hex_value(bytes[index + 1]).ok_or(
                    SearchEndpointRouteError::MalformedQuery("invalid percent escape"),
                )?;
                let low = hex_value(bytes[index + 2]).ok_or(
                    SearchEndpointRouteError::MalformedQuery("invalid percent escape"),
                )?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| SearchEndpointRouteError::InvalidUtf8)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_u32_parameter(
    parameter: &'static str,
    value: &str,
) -> Result<u32, SearchEndpointRouteError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SearchEndpointRouteError::InvalidUnsignedInteger { parameter });
    }
    value
        .parse::<u32>()
        .map_err(|_| SearchEndpointRouteError::InvalidUnsignedInteger { parameter })
}

/// Canonical query source exposed by the first Search Endpoint binding slice.
///
/// The public GET decoder maps its deliberately small parameter set into this
/// representation before canonical request construction. Richer endpoint
/// controls must remain explicit additions rather than generic pass-throughs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SearchEndpointQuery {
    Text(String),
    MatchAll,
}

/// The first typed Search Endpoint request.
///
/// Presence is intentional: `None` means the public caller did not supply that
/// control. Policy may supply an explicit default; otherwise the field remains
/// absent in the canonical SearchRequest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SearchEndpointRequest {
    pub(crate) query: SearchEndpointQuery,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

impl SearchEndpointRequest {
    pub(crate) fn text(query: impl Into<String>) -> Self {
        Self {
            query: SearchEndpointQuery::Text(query.into()),
            limit: None,
            offset: None,
        }
    }

    pub(crate) fn match_all() -> Self {
        Self {
            query: SearchEndpointQuery::MatchAll,
            limit: None,
            offset: None,
        }
    }
}

/// Fail-closed endpoint binding errors.
///
/// Canonical Search Kernel semantic errors are deliberately not represented
/// here; they occur later when the encoded request is dispatched through the
/// existing canonical path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchEndpointBindError {
    InvalidPolicy(&'static str),
    QueryTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    LimitTooLarge {
        value: u32,
        max: u32,
    },
    OffsetTooLarge {
        value: u32,
        max: u32,
    },
}

impl std::fmt::Display for SearchEndpointBindError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPolicy(message) => {
                write!(formatter, "invalid Search Endpoint policy: {message}")
            }
            Self::QueryTooLarge {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "Search Endpoint query is {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::LimitTooLarge { value, max } => write!(
                formatter,
                "Search Endpoint limit {value} exceeds maximum {max}"
            ),
            Self::OffsetTooLarge { value, max } => write!(
                formatter,
                "Search Endpoint offset {value} exceeds maximum {max}"
            ),
        }
    }
}

impl std::error::Error for SearchEndpointBindError {}

/// Validates public serving policy and constructs the canonical search
/// request envelope consumed by `searchkernel-dispatch`.
///
/// This function intentionally does not dispatch. The daemon owns scheduling,
/// admission, blocking execution, and target lifecycle; this module owns only
/// endpoint policy plus canonical request construction.
pub(crate) fn bind_search_request(
    target: &str,
    policy: &SearchEndpointPolicy,
    request: SearchEndpointRequest,
) -> Result<Vec<u8>, SearchEndpointBindError> {
    policy.validate()?;

    if let SearchEndpointQuery::Text(text) = &request.query {
        let actual_bytes = text.len();
        if actual_bytes > policy.max_query_bytes {
            return Err(SearchEndpointBindError::QueryTooLarge {
                actual_bytes,
                max_bytes: policy.max_query_bytes,
            });
        }
    }

    let limit = request.limit.or(policy.default_limit);
    if let Some(value) = limit {
        if value > policy.max_limit {
            return Err(SearchEndpointBindError::LimitTooLarge {
                value,
                max: policy.max_limit,
            });
        }
    }

    if let Some(value) = request.offset {
        if value > policy.max_offset {
            return Err(SearchEndpointBindError::OffsetTooLarge {
                value,
                max: policy.max_offset,
            });
        }
    }

    Ok(encode_search_request(
        target,
        request.query,
        limit,
        request.offset,
    ))
}

fn encode_search_request(
    target: &str,
    query: SearchEndpointQuery,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();

    let query = match query {
        SearchEndpointQuery::Text(text) => {
            let text = builder.create_string(&text);
            let text_query = api::SearchKernelTextQuery::create(
                &mut builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            api::QuerySource::create(
                &mut builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::SearchKernelTextQuery,
                    source: Some(text_query.as_union_value()),
                },
            )
        }
        SearchEndpointQuery::MatchAll => {
            let match_all =
                api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs::default());
            api::QuerySource::create(
                &mut builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::MatchAllQuery,
                    source: Some(match_all.as_union_value()),
                },
            )
        }
    };

    let limit = limit.map(api::OptionalUInt32::new);
    let offset = offset.map(api::OptionalUInt32::new);
    let search = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            limit: limit.as_ref(),
            offset: offset.as_ref(),
            ..Default::default()
        },
    );
    let target_name = builder.create_string(target);
    let target = api::IndexTarget::create(
        &mut builder,
        &api::IndexTargetArgs {
            name: Some(target_name),
            generation: None,
        },
    );
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_SearchRequest,
            payload: Some(search.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

//
// Production public Search Endpoint surface.
//
// InstantSearch and other public integrations must enter through this module.
// Keeping the listener, exposure policy, canonical binding, projection, and
// hardened concurrency ownership together prevents a second daemon-side
// semantic authority from drifting away from Search Endpoints.
//
pub mod instantsearch_public {
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
                        .any(|f| !endpoint.fields.contains(f) || f.contains('.'))
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
            let json =
                serde_json::to_vec(&envelope).map_err(|_| PublicSearchError::InvalidRequest)?;
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
        let attribute =
            json!({"expr_type": "GroupingAttributeExpression", "expr": {"field": field}});
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
            V::List(values) => {
                Value::Array(values.iter().map(result_value).collect::<Result<_, _>>()?)
            }
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
        let exhaustive_facets = result.total_hits.relation
            == searchkernel_ir::TotalHitsRelation::Exact
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
}

pub use instantsearch_public::*;
