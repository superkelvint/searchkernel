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
