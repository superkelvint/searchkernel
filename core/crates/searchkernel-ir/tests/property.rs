use crate::wire::v1::schema as api;
use crate::wire::v1::*;
use crate::*;
use std::collections::BTreeMap;

const CASES: u64 = 192;

#[derive(Clone, Debug)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    fn usize(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        (self.next_u64() % upper as u64) as usize
    }

    fn u16(&mut self) -> u16 {
        self.next_u64() as u16
    }

    fn i16(&mut self) -> i16 {
        self.next_u64() as i16
    }

    fn i32(&mut self) -> i32 {
        self.next_u64() as i32
    }

    fn i64(&mut self) -> i64 {
        self.next_u64() as i64
    }
}

fn field(
    name: &str,
    kind: RustFieldKind,
    stored: Option<bool>,
    filterable: Option<bool>,
    sortable: Option<bool>,
    facetable: Option<bool>,
) -> RustFieldInput {
    RustFieldInput {
        name: name.to_owned(),
        kind,
        multi: false,
        stored,
        searchable: None,
        filterable,
        sortable,
        facetable,
        analyzer: None,
        distance_metric: None,
        hnsw: None,
    }
}

fn context() -> IndexContext {
    let definition = resolve_rust_index_definition(RustIndexDefinitionInput {
        name: Some("properties".to_owned()),
        fields: vec![
            field(
                "brand",
                RustFieldKind::Keyword,
                Some(true),
                Some(true),
                Some(true),
                Some(true),
            ),
            field(
                "price",
                RustFieldKind::Int64,
                Some(true),
                Some(true),
                Some(true),
                None,
            ),
            field(
                "rating",
                RustFieldKind::Float64,
                Some(true),
                Some(true),
                Some(true),
                None,
            ),
            field(
                "in_stock",
                RustFieldKind::Bool,
                Some(true),
                Some(true),
                None,
                None,
            ),
        ],
        default_search_fields: None,
    })
    .expect("property-test schema resolves");
    IndexContext::from_definition_ir(definition)
}

fn base_search() -> SearchInput {
    SearchInput {
        query: QuerySourceInput::MatchAll,
        search_fields: None,
        filters: Vec::new(),
        textual_filters: Vec::new(),
        sorts: Vec::new(),
        grouping: Vec::new(),
        ranking: None,
        query_inputs: BTreeMap::new(),
        rank_properties: BTreeMap::new(),
        query_model: QueryModelInput::default(),
        projection: None,
        highlighting: None,
        presentation: PresentationInput::default(),
        properties: BTreeMap::new(),
        query_profile: None,
        search_chain: None,
        timeout: None,
        limit: None,
        offset: None,
    }
}

fn generated_ascii(rng: &mut Rng, max_len: usize) -> String {
    let len = rng.usize(max_len + 1);
    (0..len)
        .map(|_| char::from(b'a' + rng.usize(26) as u8))
        .collect()
}

fn generated_request_value(rng: &mut Rng, depth: usize) -> RequestValueIr {
    let leaf_kinds = if depth == 0 { 8 } else { 10 };
    match rng.usize(leaf_kinds) {
        0 => RequestValueIr::Null,
        1 => RequestValueIr::Bool(rng.next_bool()),
        2 => RequestValueIr::Int64(rng.i64()),
        3 => RequestValueIr::UInt64(rng.next_u64()),
        4 => RequestValueIr::Float64(rng.i32() as f64 / 17.0),
        5 => RequestValueIr::String(generated_ascii(rng, 16)),
        6 => RequestValueIr::Bytes((0..rng.usize(12)).map(|_| rng.next_u64() as u8).collect()),
        7 => RequestValueIr::NonFiniteFloat(match rng.usize(3) {
            0 => RequestNonFiniteFloatIr::NaN,
            1 => RequestNonFiniteFloatIr::PositiveInfinity,
            _ => RequestNonFiniteFloatIr::NegativeInfinity,
        }),
        8 => {
            let len = rng.usize(4);
            RequestValueIr::List(
                (0..len)
                    .map(|_| generated_request_value(rng, depth - 1))
                    .collect(),
            )
        }
        _ => {
            let len = rng.usize(4);
            let mut values = BTreeMap::new();
            for index in 0..len {
                values.insert(format!("k{index}"), generated_request_value(rng, depth - 1));
            }
            RequestValueIr::Object(values)
        }
    }
}

fn generated_properties(rng: &mut Rng) -> BTreeMap<String, RequestValueIr> {
    let len = rng.usize(5);
    let mut properties = BTreeMap::new();
    for index in 0..len {
        properties.insert(format!("p{index}"), generated_request_value(rng, 3));
    }
    properties
}

#[derive(Clone, Debug)]
struct SchemaFieldSpec {
    kind: u8,
    stored: bool,
    filterable: bool,
}

fn generated_schema_specs(rng: &mut Rng) -> Vec<SchemaFieldSpec> {
    let len = 1 + rng.usize(6);
    (0..len)
        .map(|_| SchemaFieldSpec {
            kind: rng.usize(4) as u8,
            stored: rng.next_bool(),
            filterable: rng.next_bool(),
        })
        .collect()
}

fn schema_input(specs: &[SchemaFieldSpec], explicit_false: bool) -> RustIndexDefinitionInput {
    let fields = specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let kind = match spec.kind {
                0 => RustFieldKind::Keyword,
                1 => RustFieldKind::Int64,
                2 => RustFieldKind::Float64,
                _ => RustFieldKind::Bool,
            };
            let option = |value: bool| {
                if explicit_false {
                    Some(value)
                } else if value {
                    Some(true)
                } else {
                    None
                }
            };
            field(
                &format!("f{index}"),
                kind,
                option(spec.stored),
                option(spec.filterable),
                None,
                None,
            )
        })
        .collect();

    RustIndexDefinitionInput {
        name: Some("generated".to_owned()),
        fields,
        default_search_fields: None,
    }
}

fn default_equivalent_schema_input(
    specs: &[SchemaFieldSpec],
    explicit_defaults: bool,
) -> RustIndexDefinitionInput {
    let fields = specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let kind = match spec.kind {
                0 => RustFieldKind::Keyword,
                1 => RustFieldKind::Int64,
                2 => RustFieldKind::Float64,
                _ => RustFieldKind::Bool,
            };
            field(
                &format!("f{index}"),
                kind,
                explicit_defaults.then_some(true),
                explicit_defaults.then_some(true),
                explicit_defaults.then_some(true),
                None,
            )
        })
        .collect();

    RustIndexDefinitionInput {
        name: Some("generated".to_owned()),
        fields,
        default_search_fields: None,
    }
}

fn decode_schema(bytes: &[u8]) -> IndexDefinitionIr {
    let portable = crate::__flatbuffers::root::<api::IndexDefinition>(bytes)
        .expect("generated schema bytes decode");
    index_definition_to_ir(portable).expect("generated schema resolves")
}

fn decode_search(context: &IndexContext, bytes: &[u8]) -> SearchRequestIr {
    let portable = crate::__flatbuffers::root::<api::SearchRequest>(bytes)
        .expect("generated search bytes decode");
    context
        .search_to_ir(portable)
        .expect("generated search resolves")
}

fn round_trip_search(context: &IndexContext, request: &SearchRequestIr) -> SearchRequestIr {
    decode_search(context, &search_request_ir_to_api(request))
}

fn parameter(value: Value) -> FilterParameters {
    BTreeMap::from([("v".to_owned(), value)])
}

fn i64_is_lossless_f64(value: i64) -> bool {
    let converted = value as f64;
    (value < 0 || converted < 2_f64.powi(63)) && converted as i64 == value
}

fn u64_is_lossless_f64(value: u64) -> bool {
    let converted = value as f64;
    converted < 2_f64.powi(64) && converted as u64 == value
}

fn f64_is_lossless_i64(value: f64) -> bool {
    value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value < 2_f64.powi(63)
        && (value as i64) as f64 == value
}

#[derive(Clone, Debug)]
enum BoolFilterSpec {
    PriceEq(i64),
    StockEq(bool),
    All(Vec<BoolFilterSpec>),
    Any(Vec<BoolFilterSpec>),
    Not(Box<BoolFilterSpec>),
}

fn generated_filter_spec(rng: &mut Rng, depth: usize) -> BoolFilterSpec {
    if depth == 0 || rng.usize(5) < 2 {
        return if rng.next_bool() {
            BoolFilterSpec::PriceEq(i64::from(rng.i16()))
        } else {
            BoolFilterSpec::StockEq(rng.next_bool())
        };
    }
    match rng.usize(3) {
        0 => {
            let len = 1 + rng.usize(3);
            BoolFilterSpec::All(
                (0..len)
                    .map(|_| generated_filter_spec(rng, depth - 1))
                    .collect(),
            )
        }
        1 => {
            let len = 1 + rng.usize(3);
            BoolFilterSpec::Any(
                (0..len)
                    .map(|_| generated_filter_spec(rng, depth - 1))
                    .collect(),
            )
        }
        _ => BoolFilterSpec::Not(Box::new(generated_filter_spec(rng, depth - 1))),
    }
}

fn filter_input(spec: &BoolFilterSpec) -> FilterInput {
    match spec {
        BoolFilterSpec::PriceEq(value) => FilterInput::Comparison {
            field: "price".to_owned(),
            op: ComparisonOp::Eq,
            value: RequestValueIr::Int64(*value),
        },
        BoolFilterSpec::StockEq(value) => FilterInput::Comparison {
            field: "in_stock".to_owned(),
            op: ComparisonOp::Eq,
            value: RequestValueIr::Bool(*value),
        },
        BoolFilterSpec::All(values) => FilterInput::All(values.iter().map(filter_input).collect()),
        BoolFilterSpec::Any(values) => FilterInput::Any(values.iter().map(filter_input).collect()),
        BoolFilterSpec::Not(value) => FilterInput::Not(Box::new(filter_input(value))),
    }
}

fn eval_filter_spec(spec: &BoolFilterSpec, price: i64, in_stock: bool) -> bool {
    match spec {
        BoolFilterSpec::PriceEq(value) => price == *value,
        BoolFilterSpec::StockEq(value) => in_stock == *value,
        BoolFilterSpec::All(values) => values
            .iter()
            .all(|value| eval_filter_spec(value, price, in_stock)),
        BoolFilterSpec::Any(values) => values
            .iter()
            .any(|value| eval_filter_spec(value, price, in_stock)),
        BoolFilterSpec::Not(value) => !eval_filter_spec(value, price, in_stock),
    }
}

fn eval_filter_ir(value: &FilterIr, price: i64, in_stock: bool) -> bool {
    match value {
        FilterIr::And(values) => values
            .iter()
            .all(|value| eval_filter_ir(value, price, in_stock)),
        FilterIr::Or(values) => values
            .iter()
            .any(|value| eval_filter_ir(value, price, in_stock)),
        FilterIr::Not(value) => !eval_filter_ir(value, price, in_stock),
        FilterIr::Comparison(value) => match (value.field().as_str(), value.op(), value.value()) {
            ("price", ComparisonOp::Eq, ScalarValueIr::Int64(expected)) => price == *expected,
            ("in_stock", ComparisonOp::Eq, ScalarValueIr::Bool(expected)) => in_stock == *expected,
            other => panic!("unexpected generated comparison {other:?}"),
        },
        other => panic!("unexpected generated filter {other:?}"),
    }
}

#[derive(Clone, Debug)]
enum BoolQuerySpec {
    True,
    False,
    And(Vec<BoolQuerySpec>),
    Or(Vec<BoolQuerySpec>),
    Not(Box<BoolQuerySpec>),
}

fn generated_query_spec(rng: &mut Rng, depth: usize) -> BoolQuerySpec {
    if depth == 0 || rng.usize(5) < 2 {
        return if rng.next_bool() {
            BoolQuerySpec::True
        } else {
            BoolQuerySpec::False
        };
    }
    match rng.usize(3) {
        0 => {
            let len = 1 + rng.usize(3);
            BoolQuerySpec::And(
                (0..len)
                    .map(|_| generated_query_spec(rng, depth - 1))
                    .collect(),
            )
        }
        1 => {
            let len = 1 + rng.usize(3);
            BoolQuerySpec::Or(
                (0..len)
                    .map(|_| generated_query_spec(rng, depth - 1))
                    .collect(),
            )
        }
        _ => BoolQuerySpec::Not(Box::new(generated_query_spec(rng, depth - 1))),
    }
}

fn query_expr(spec: &BoolQuerySpec) -> QueryExprIr {
    let kind = match spec {
        BoolQuerySpec::True => QueryExprKindIr::True,
        BoolQuerySpec::False => QueryExprKindIr::False,
        BoolQuerySpec::And(values) => QueryExprKindIr::And(values.iter().map(query_expr).collect()),
        BoolQuerySpec::Or(values) => QueryExprKindIr::Or(values.iter().map(query_expr).collect()),
        BoolQuerySpec::Not(value) => QueryExprKindIr::Not(Box::new(query_expr(value))),
    };
    QueryExprIr {
        kind,
        annotations: BTreeMap::new(),
    }
}

fn eval_query_spec(spec: &BoolQuerySpec) -> bool {
    match spec {
        BoolQuerySpec::True => true,
        BoolQuerySpec::False => false,
        BoolQuerySpec::And(values) => values.iter().all(eval_query_spec),
        BoolQuerySpec::Or(values) => values.iter().any(eval_query_spec),
        BoolQuerySpec::Not(value) => !eval_query_spec(value),
    }
}

fn eval_query_ir(value: &QueryExprIr) -> bool {
    match &value.kind {
        QueryExprKindIr::True => true,
        QueryExprKindIr::False => false,
        QueryExprKindIr::And(values) => values.iter().all(eval_query_ir),
        QueryExprKindIr::Or(values) => values.iter().any(eval_query_ir),
        QueryExprKindIr::Not(value) => !eval_query_ir(value),
        other => panic!("unexpected generated boolean query {other:?}"),
    }
}

#[derive(Clone, Debug)]
enum GroupingOpSpec {
    Precision(u16),
    Max(u16),
    Where(i16),
    Order(i16),
}

fn generated_grouping_request(rng: &mut Rng) -> GroupingRequestIr {
    let len = rng.usize(12);
    let operations = (0..len)
        .map(|_| match rng.usize(4) {
            0 => GroupingOpSpec::Precision(rng.u16()),
            1 => GroupingOpSpec::Max(rng.u16()),
            2 => GroupingOpSpec::Where(rng.i16()),
            _ => GroupingOpSpec::Order(rng.i16()),
        })
        .map(|op| match op {
            GroupingOpSpec::Precision(value) => GroupingOperationIr::Precision(u32::from(value)),
            GroupingOpSpec::Max(value) => GroupingOperationIr::Max(Some(u32::from(value))),
            GroupingOpSpec::Where(value) => GroupingOperationIr::Where(
                GroupingExpressionIr::Literal(RequestValueIr::Int64(i64::from(value))),
            ),
            GroupingOpSpec::Order(value) => {
                GroupingOperationIr::Order(vec![GroupingExpressionIr::Literal(
                    RequestValueIr::Int64(i64::from(value)),
                )])
            }
        })
        .collect();

    GroupingRequestIr {
        root: GroupingAllIr {
            operations,
            label: Some("root".to_owned()),
        },
        continuations: Vec::new(),
    }
}

fn generated_facet_sort(rng: &mut Rng) -> FacetSortInput {
    match rng.usize(4) {
        0 => FacetSortInput::CountDesc,
        1 => FacetSortInput::CountAsc,
        2 => FacetSortInput::ValueAsc,
        _ => FacetSortInput::ValueDesc,
    }
}

fn with_seed(seed: u64, property: impl FnOnce(&mut Rng)) {
    let mut rng = Rng::new(seed);
    property(&mut rng);
}

#[test]
fn property_api_ir_api_ir_is_stable_for_generated_supported_schemas() {
    for seed in 0..CASES {
        with_seed(seed ^ 0x01, |rng| {
            let specs = generated_schema_specs(rng);
            let resolved = resolve_rust_index_definition(schema_input(&specs, false))
                .unwrap_or_else(|error| panic!("seed {seed}: generated schema failed: {error:?}"));

            let first = decode_schema(&index_definition_ir_to_api(&resolved));
            let second = decode_schema(&index_definition_ir_to_api(&first));

            assert_eq!(first, second, "seed {seed}");
            assert_eq!(
                snapshot::index_definition(&first),
                snapshot::index_definition(&second),
                "seed {seed}",
            );
        });
    }
}

#[test]
fn property_schema_snapshot_generation_is_deterministic() {
    for seed in 0..CASES {
        with_seed(seed ^ 0x02, |rng| {
            let specs = generated_schema_specs(rng);
            let first = resolve_rust_index_definition(schema_input(&specs, false)).unwrap();
            let second = resolve_rust_index_definition(schema_input(&specs, false)).unwrap();
            let first_snapshot = snapshot::index_definition(&first);
            let second_snapshot = snapshot::index_definition(&second);

            assert_eq!(first_snapshot, second_snapshot, "seed {seed}");
            assert_eq!(
                serde_json::to_vec(&first_snapshot).unwrap(),
                serde_json::to_vec(&second_snapshot).unwrap(),
                "seed {seed}",
            );
        });
    }
}

#[test]
fn property_equivalent_normalized_schemas_have_stable_snapshots() {
    for seed in 0..CASES {
        with_seed(seed ^ 0x03, |rng| {
            let specs = generated_schema_specs(rng);
            let implicit =
                resolve_rust_index_definition(default_equivalent_schema_input(&specs, false))
                    .unwrap();
            let explicit =
                resolve_rust_index_definition(default_equivalent_schema_input(&specs, true))
                    .unwrap();

            assert_eq!(
                snapshot::index_definition(&implicit),
                snapshot::index_definition(&explicit),
                "seed {seed}: implicit defaults and explicit defaults diverged",
            );
        });
    }
}

#[test]
fn property_encode_decode_preserves_supported_portable_values() {
    for seed in 0..CASES {
        with_seed(seed ^ 0x04, |rng| {
            let context = context();
            let properties = generated_properties(rng);
            let limit = rng.next_u64() as u32;
            let offset = rng.next_u64() as u32;
            let mut input = base_search();
            input.properties = properties.clone();
            input.limit = Some(limit);
            input.offset = Some(offset);
            let resolved = resolve_search(&context, input).unwrap();
            let decoded = round_trip_search(&context, &resolved);
            let decoded_again = round_trip_search(&context, &decoded);

            assert_eq!(decoded.properties(), &properties, "seed {seed}");
            assert_eq!(decoded.limit(), limit, "seed {seed}");
            assert_eq!(decoded.offset(), offset, "seed {seed}");
            assert_eq!(resolved, decoded, "seed {seed}");
            assert_eq!(
                decoded, decoded_again,
                "seed {seed}: API -> IR -> API -> IR changed meaning",
            );
        });
    }
}

#[test]
fn property_presence_sensitive_fields_preserve_absent_vs_present_empty() {
    for seed in 0..CASES {
        with_seed(seed ^ 0x05, |rng| {
            let context = context();
            let mut absent_input = base_search();
            absent_input.properties = generated_properties(rng);
            absent_input.limit = Some(rng.next_u64() as u32);
            absent_input.offset = Some(rng.next_u64() as u32);

            let absent = resolve_search(&context, absent_input.clone()).unwrap();

            let mut empty_projection_input = absent_input.clone();
            empty_projection_input.projection = Some(Vec::new());
            let empty_projection = resolve_search(&context, empty_projection_input).unwrap();

            let mut empty_highlighting_input = absent_input;
            empty_highlighting_input.highlighting = Some(Vec::new());
            let empty_highlighting = resolve_search(&context, empty_highlighting_input).unwrap();

            assert_ne!(
                absent, empty_projection,
                "seed {seed}: absent and present-empty projection collapsed"
            );
            assert_ne!(
                absent, empty_highlighting,
                "seed {seed}: absent and present-empty highlighting collapsed"
            );
            assert!(matches!(absent.highlighting(), HighlightingIr::Disabled));
            assert!(matches!(
                empty_highlighting.highlighting(),
                HighlightingIr::Requested {
                    fields,
                    multi_valued_fields,
                } if fields.is_empty() && multi_valued_fields.is_empty()
            ));

            assert_eq!(
                round_trip_search(&context, &absent),
                absent,
                "seed {seed}: absent presence state changed on round trip"
            );
            assert_eq!(
                round_trip_search(&context, &empty_projection),
                empty_projection,
                "seed {seed}: present-empty projection changed on round trip"
            );
            assert_eq!(
                round_trip_search(&context, &empty_highlighting),
                empty_highlighting,
                "seed {seed}: present-empty highlighting changed on round trip"
            );
        });
    }
}

#[test]
fn property_numeric_coercion_accepts_only_lossless_legal_conversions() {
    let context = context();

    let signed_edges = [
        i64::MIN,
        i64::MIN + 1,
        -(1_i64 << 53) - 1,
        -(1_i64 << 53),
        -1,
        0,
        1,
        (1_i64 << 53),
        (1_i64 << 53) + 1,
        i64::MAX - 1,
        i64::MAX,
    ];
    for (case, value) in signed_edges.into_iter().enumerate() {
        let result = bind_filter_expression(
            "rating = :v",
            context.definition(),
            &parameter(Value::Int64(value)),
        );
        assert_eq!(
            result.is_ok(),
            i64_is_lossless_f64(value),
            "signed edge case {case}: {value}",
        );
    }

    let unsigned_edges = [
        0,
        1,
        (1_u64 << 53),
        (1_u64 << 53) + 1,
        i64::MAX as u64,
        i64::MAX as u64 + 1,
        u64::MAX - 1,
        u64::MAX,
    ];
    for (case, value) in unsigned_edges.into_iter().enumerate() {
        let integer = bind_filter_expression(
            "price = :v",
            context.definition(),
            &parameter(Value::UInt64(value)),
        );
        assert_eq!(
            integer.is_ok(),
            value <= i64::MAX as u64,
            "unsigned->int edge case {case}: {value}",
        );

        let float = bind_filter_expression(
            "rating = :v",
            context.definition(),
            &parameter(Value::UInt64(value)),
        );
        assert_eq!(
            float.is_ok(),
            u64_is_lossless_f64(value),
            "unsigned->float edge case {case}: {value}",
        );
    }

    for seed in 0..CASES {
        with_seed(seed ^ 0x06, |rng| {
            let signed = rng.i64();
            let signed_result = bind_filter_expression(
                "rating = :v",
                context.definition(),
                &parameter(Value::Int64(signed)),
            );
            assert_eq!(
                signed_result.is_ok(),
                i64_is_lossless_f64(signed),
                "seed {seed}: signed value {signed}",
            );

            let unsigned = rng.next_u64();
            let unsigned_result = bind_filter_expression(
                "rating = :v",
                context.definition(),
                &parameter(Value::UInt64(unsigned)),
            );
            assert_eq!(
                unsigned_result.is_ok(),
                u64_is_lossless_f64(unsigned),
                "seed {seed}: unsigned value {unsigned}",
            );
        });
    }
}

#[test]
fn property_integer_coercion_rejects_fractional_values() {
    let context = context();
    for seed in 0..CASES {
        with_seed(seed ^ 0x07, |rng| {
            let base = i64::from(rng.i32() % 1_000_000);
            let numerator = 1 + (rng.u16() % 999);
            let value = base as f64 + f64::from(numerator) / 1000.0;
            assert_ne!(value.fract(), 0.0, "seed {seed}");

            let error = bind_filter_expression(
                "price = :v",
                context.definition(),
                &parameter(Value::Float64(value)),
            )
            .expect_err("fractional values cannot coerce to integer fields");
            assert_eq!(error.details[0].code, "type_mismatch", "seed {seed}");
        });
    }
}

#[test]
fn property_float_to_integer_coercion_is_exact_or_rejected() {
    let context = context();
    let edge_values = [
        i64::MIN as f64,
        i64::MIN as f64 - 1024.0,
        -9_007_199_254_740_992.0,
        -1.0,
        -0.0,
        0.0,
        1.0,
        9_007_199_254_740_992.0,
        9_007_199_254_740_993.0,
        2_f64.powi(63),
    ];
    for (case, value) in edge_values.into_iter().enumerate() {
        let result = bind_filter_expression(
            "price = :v",
            context.definition(),
            &parameter(Value::Float64(value)),
        );
        assert_eq!(
            result.is_ok(),
            f64_is_lossless_i64(value),
            "float edge case {case}: {value:?}",
        );
    }

    for seed in 0..CASES {
        with_seed(seed ^ 0x08, |rng| {
            let integral = rng.i32() as f64;
            let fractional = integral + 0.5;
            for value in [integral, fractional] {
                let result = bind_filter_expression(
                    "price = :v",
                    context.definition(),
                    &parameter(Value::Float64(value)),
                );
                assert_eq!(
                    result.is_ok(),
                    f64_is_lossless_i64(value),
                    "seed {seed}: {value:?}",
                );
            }
        });
    }
}

#[test]
fn property_non_finite_floats_are_rejected_where_forbidden() {
    let context = context();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = bind_filter_expression(
            "rating = :v",
            context.definition(),
            &parameter(Value::Float64(value)),
        )
        .expect_err("non-finite numeric parameters must be rejected");
        assert_eq!(error.details[0].code, "non_finite_number");
    }
}

#[test]
fn property_filter_clause_local_bindings_never_leak() {
    let context = context();
    for seed in 0..CASES {
        with_seed(seed ^ 0x09, |rng| {
            let left = i64::from(rng.i32());
            let mut right = i64::from(rng.i32());
            if left == right {
                right = right.wrapping_add(1);
            }

            let mut input = base_search();
            input.textual_filters = vec![
                TextualFilterInput {
                    source: "price = :v".to_owned(),
                    bindings: parameter(Value::Int64(left)),
                },
                TextualFilterInput {
                    source: "price = :v".to_owned(),
                    bindings: parameter(Value::Int64(right)),
                },
            ];
            let resolved = resolve_search(&context, input).unwrap();
            let expected_left = bind_filter_expression(
                "price = :v",
                context.definition(),
                &parameter(Value::Int64(left)),
            )
            .unwrap();
            let expected_right = bind_filter_expression(
                "price = :v",
                context.definition(),
                &parameter(Value::Int64(right)),
            )
            .unwrap();

            assert_eq!(
                resolved.filter(),
                Some(&FilterIr::And(vec![expected_left, expected_right])),
                "seed {seed}: clause-local binding leaked",
            );
        });
    }
}

#[test]
fn property_pagination_never_overflows_and_normalizes_deterministically() {
    let context = context();
    let extreme_pairs = [
        (0, 0),
        (0, u32::MAX),
        (u32::MAX, 0),
        (u32::MAX, u32::MAX),
        (1, u32::MAX),
        (u32::MAX, 1),
    ];
    for (limit, offset) in extreme_pairs {
        let mut input = base_search();
        input.limit = Some(limit);
        input.offset = Some(offset);
        let resolved = resolve_search(&context, input).unwrap();
        let decoded = round_trip_search(&context, &resolved);
        assert_eq!(resolved.limit(), limit);
        assert_eq!(resolved.offset(), offset);
        assert_eq!(resolved, decoded);
    }

    for seed in 0..CASES {
        with_seed(seed ^ 0x0A, |rng| {
            let page = (rng.next_u64() as u32) & 0xFFFF;
            let size = 1 + ((rng.next_u64() as u32) & 0xFFFF);
            let offset = page
                .checked_mul(size)
                .expect("bounded generated page arithmetic cannot overflow");

            let mut input = base_search();
            input.limit = Some(size);
            input.offset = Some(offset);
            let first = resolve_search(&context, input.clone()).unwrap();
            let second = resolve_search(&context, input).unwrap();

            assert_eq!(first.limit(), size, "seed {seed}");
            assert_eq!(first.offset(), offset, "seed {seed}");
            assert_eq!(
                snapshot::search_request(&first),
                snapshot::search_request(&second),
                "seed {seed}",
            );
        });
    }
}

#[test]
fn property_boolean_filter_normalization_preserves_tree_meaning() {
    let context = context();
    for seed in 0..CASES {
        with_seed(seed ^ 0x0B, |rng| {
            let spec = generated_filter_spec(rng, 4);
            let price = i64::from(rng.i16());
            let in_stock = rng.next_bool();

            let mut input = base_search();
            input.filters = vec![filter_input(&spec)];
            let resolved = resolve_search(&context, input).unwrap();
            let actual = resolved.filter().expect("generated filter is present");

            assert_eq!(
                eval_filter_spec(&spec, price, in_stock),
                eval_filter_ir(actual, price, in_stock),
                "seed {seed}: {spec:?}",
            );
        });
    }
}

#[test]
fn property_boolean_query_normalization_preserves_tree_meaning() {
    let context = context();
    for seed in 0..CASES {
        with_seed(seed ^ 0x0C, |rng| {
            let spec = generated_query_spec(rng, 4);
            let mut input = base_search();
            input.query = QuerySourceInput::Typed(query_expr(&spec));
            let resolved = resolve_search(&context, input).unwrap();
            let QueryIr::Typed(actual) = resolved.query() else {
                panic!("seed {seed}: typed query must remain typed");
            };

            assert_eq!(
                eval_query_spec(&spec),
                eval_query_ir(actual),
                "seed {seed}: {spec:?}",
            );
        });
    }
}

#[test]
fn property_grouping_encode_decode_preserves_child_order() {
    let context = context();
    for seed in 0..CASES {
        with_seed(seed ^ 0x0D, |rng| {
            let grouping = generated_grouping_request(rng);
            let mut input = base_search();
            input.grouping = vec![GroupingInput::Structured(grouping.clone())];

            let resolved = resolve_search(&context, input).unwrap();
            assert_eq!(
                resolved.grouping(),
                std::slice::from_ref(&grouping),
                "seed {seed}"
            );

            let decoded = round_trip_search(&context, &resolved);
            assert_eq!(decoded.grouping(), &[grouping], "seed {seed}");
        });
    }
}

#[test]
fn property_facet_convenience_always_lowers_to_documented_grouping_shape() {
    let context = context();
    for seed in 0..CASES {
        with_seed(seed ^ 0x0E, |rng| {
            let facet = FacetInput {
                field: "brand".to_owned(),
                name: rng
                    .next_bool()
                    .then(|| format!("facet_{}", rng.usize(10_000))),
                limit: rng.next_bool().then(|| u32::from(rng.u16())),
                sort: Some(generated_facet_sort(rng)),
            };
            let expected = facet_grouping_input(&facet);
            let actual = resolve_facets(&context, vec![facet]).unwrap();
            assert_eq!(actual, vec![expected], "seed {seed}");
        });
    }
}

#[test]
fn property_duplicate_aliases_fields_and_operators_are_rejected_deterministically() {
    let context = context();
    let operators = ["=", "!=", "<", "<=", ">", ">="];

    for seed in 0..CASES {
        with_seed(seed ^ 0x0F, |rng| {
            let alias = format!("a{}", rng.usize(10_000));
            let value = i64::from(rng.i16());
            let duplicate_alias = GroupingRequestIr {
                root: GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Alias {
                            name: alias.clone(),
                            expression: GroupingExpressionIr::Literal(RequestValueIr::Int64(value)),
                        },
                        GroupingOperationIr::Alias {
                            name: alias,
                            expression: GroupingExpressionIr::Literal(RequestValueIr::Int64(value)),
                        },
                    ],
                    label: None,
                },
                continuations: Vec::new(),
            };
            let mut alias_input = base_search();
            alias_input.grouping = vec![GroupingInput::Structured(duplicate_alias)];
            let alias_error = resolve_search(&context, alias_input)
                .expect_err("duplicate grouping aliases must fail");
            assert_eq!(
                alias_error.details[0].code, "duplicate_grouping_alias",
                "seed {seed}",
            );

            let duplicate_fields = resolve_facets(
                &context,
                vec![
                    FacetInput {
                        field: "brand".to_owned(),
                        name: None,
                        limit: None,
                        sort: None,
                    },
                    FacetInput {
                        field: "brand".to_owned(),
                        name: None,
                        limit: None,
                        sort: None,
                    },
                ],
            )
            .expect_err("duplicate facet fields must fail");
            assert_eq!(
                duplicate_fields.details[0].code, "duplicate_facet_field",
                "seed {seed}",
            );

            let operator = operators[rng.usize(operators.len())];
            let source = format!("price {operator} {operator} 1");
            let first =
                bind_filter_expression(&source, context.definition(), &FilterParameters::new())
                    .expect_err("duplicate comparison operators must fail");
            let second =
                bind_filter_expression(&source, context.definition(), &FilterParameters::new())
                    .expect_err("same duplicate comparison operators must fail deterministically");

            assert_eq!(first.code, second.code, "seed {seed}: {source}");
            assert_eq!(
                first.details[0].code, second.details[0].code,
                "seed {seed}: {source}",
            );
            assert_eq!(
                first.details[0].path, second.details[0].path,
                "seed {seed}: {source}",
            );
        });
    }
}
