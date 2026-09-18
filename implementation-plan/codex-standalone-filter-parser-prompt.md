# Codex Task: Implement the Search Kernel Filter Parser as a Standalone Rust Crate

You are implementing only the **syntax layer** of Search Kernel's filter expression language.

You do **not** have access to the Search Kernel repository, its IR, schema types, FlatBuffers contract, Vespa adapter, clients, transports, or binder. That is intentional.

You are given exactly two normative inputs:

1. `query-language-spec.md`
2. `filter-parser-validation.json`

Treat `query-language-spec.md` as the language specification. Treat `filter-parser-validation.json` as the frozen executable contract for this standalone parser phase. If the JSON is more precise about parser-only representation, spans, limits, or error classification, follow the JSON. Do not modify either file to make the implementation pass.

## Goal

Produce a small, production-quality Rust crate named:

```text
searchkernel-filter
```

that implements:

```text
filter source text
    -> lexer
    -> syntax AST
```

and nothing beyond that boundary.

The parser must be fully usable and testable without Search Kernel.

A later task will integrate this crate with Search Kernel's schema-aware binder and resolved filter IR.

## Hard architectural boundary

Implement syntax only.

The parser MUST NOT:

- know whether a field exists;
- know a field's Search Kernel type;
- check whether a field is filterable;
- decide whether an operator is legal for a field type;
- coerce numeric literals into `i64`, `u64`, or `f64`;
- resolve parameter values;
- detect missing, unused, or duplicate request parameter bindings;
- generate Search Kernel resolved IR;
- generate FlatBuffers;
- generate YQL;
- contain Vespa-specific code;
- perform query execution.

For example:

```text
price <= true
```

is syntactically valid and MUST parse successfully.

Semantic rejection of that expression belongs to the later binder.

## Required crate layout

Use this shape unless a very small mechanical variation is necessary:

```text
searchkernel-filter/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── lexer.rs
│   ├── parser.rs
│   ├── ast.rs
│   ├── error.rs
│   └── limits.rs
└── tests/
    └── golden.rs
```

Do not create `bind.rs` in this task.

Keep dependencies minimal. Prefer the standard library for the parser. Test-only use of `serde` / `serde_json` to consume the frozen validation corpus is acceptable.

## Required public API

Expose:

```rust
pub fn parse_filter(source: &str) -> Result<Spanned<Expr>, ParseError>;
```

The syntax model should expose the following concepts. Exact internal organization may vary, but the public meaning must remain equivalent and the frozen fixture renderer must produce exactly the JSON representation required by `filter-parser-validation.json`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Predicate(Predicate),
    And(Vec<Spanned<Expr>>),
    Or(Vec<Spanned<Expr>>),
    Not(Box<Spanned<Expr>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    Compare {
        field: Spanned<FieldName>,
        op: CompareOp,
        value: Spanned<ValueExpr>,
    },
    In {
        field: Spanned<FieldName>,
        values: InValues,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InValues {
    Values(Vec<Spanned<ValueExpr>>),
    Parameter(Spanned<ParameterName>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueExpr {
    String(String),
    Number(NumericLiteral),
    Boolean(bool),
    Parameter(ParameterName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericLiteral {
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldName(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterName(pub String);
```

You may add methods and private helper types.

Do not replace `NumericLiteral.source` with a parsed numeric value. The exact source spelling must survive parsing.

## Grammar

Implement the v1 grammar from `query-language-spec.md`:

```ebnf
expression
    = or_expression ;

or_expression
    = and_expression
      { OR and_expression } ;

and_expression
    = not_expression
      { AND not_expression } ;

not_expression
    = NOT not_expression
    | primary ;

primary
    = "(" expression ")"
    | predicate ;

predicate
    = field comparison value
    | field IN "(" in_values ")" ;

comparison
    = "="
    | "<"
    | "<="
    | ">"
    | ">=" ;

in_values
    = parameter
    | value { "," value } ;

value
    = string
    | number
    | boolean
    | parameter ;
```

Precedence is:

```text
NOT
AND
OR
```

Parentheses override precedence.

Contiguous `AND` and `OR` expressions should be represented as n-ary `And(Vec<...>)` and `Or(Vec<...>)`, not unnecessarily nested binary trees.

## Lexer contract

Required token concepts:

```text
Identifier
StringLiteral
NumberLiteral
Parameter

Eq
Lt
Lte
Gt
Gte

And
Or
Not
In

True
False

LeftParen
RightParen
Comma
End
```

Every token carries a UTF-8 **byte** span `[start, end)`.

Keywords are case-insensitive:

```text
AND OR NOT IN TRUE FALSE
```

Identifiers and parameter names preserve original case.

The lexer performs no schema lookup.

### Standalone identifier rule

The source specification intentionally delegates field-name legality to Search Kernel's schema rules, which are unavailable in this isolated task.

For this standalone parser contract, freeze the lexical field identifier grammar to:

```regex
[A-Za-z_][A-Za-z0-9_]*
```

Parameter names use the same identifier grammar after `:`:

```regex
:[A-Za-z_][A-Za-z0-9_]*
```

Do not widen this on your own.

### Numbers

Recognize numeric literals using this standalone lexical contract:

```regex
-?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?
```

Examples:

```text
120
-5
120.5
-0.25
1e6
-1.25e+3
```

A leading `+` is not accepted.

Preserve the exact matched spelling in `NumericLiteral.source`.

Do not perform overflow checks or finite-range checks here; those are semantic binding concerns.

### Strings

Strings use single quotes.

A quote inside a string is escaped by doubling it:

```text
'O''Reilly'
```

The AST value for that literal is:

```text
O'Reilly
```

Backslash is not an escape character.

### Whitespace

Ignore Unicode whitespace outside literals. All spans remain UTF-8 byte offsets.

## `IN` disambiguation

Preserve first-class `IN`.

For:

```text
brand IN (:brands)
```

where the parenthesized content is exactly one parameter and contains no comma, produce:

```text
InValues::Parameter
```

For an actual value list, produce:

```text
InValues::Values
```

A mixed value list may contain scalar parameter expressions, for example:

```text
rating IN (1, :two, 3)
```

which is a `Values` list containing Number, Parameter, Number.

An empty list:

```text
brand IN ()
```

is a parser error with kind `empty_in`.

Do not rewrite `IN` to `OR`.

## Parentheses and spans

There is no required `Group` AST node.

When parentheses wrap an expression, keep the expression's semantic AST node but widen that expression node's outer span to include the opening and closing parentheses.

Example:

```text
(A = 1 OR B = 2)
```

produces an `Or` expression whose outer span includes both parentheses. Child predicate spans do not include the parentheses.

This rule makes diagnostics retain the complete source extent without preserving a semantically meaningless grouping node.

## Error model

Expose a structured `ParseError`.

At minimum it must distinguish:

```text
syntax_error
empty_in
expression_too_complex
```

It must carry a byte span.

For `expression_too_complex`, retain which configured limit was exceeded when practical.

The golden verifier compares stable error codes and spans, not English prose. Error messages should still be useful to a human, but wording is not part of the frozen contract.

Use these stable reason strings where the validation corpus specifies them:

```text
expected_expression
invalid_operator
unclosed_string
expected_right_paren
expected_left_paren
expected_value
expected_operator
unexpected_token
invalid_parameter
expected_in_value
```

## Complexity limits

Freeze these constants for this standalone phase:

```text
MAX_EXPRESSION_BYTES      = 65_536
MAX_TOKENS                = 16_384
MAX_NESTING_DEPTH         = 64
MAX_AST_NODES             = 4_096
MAX_IN_VALUES             = 1_024
MAX_PARAMETER_REFERENCES  = 1_024
```

Definitions:

- `MAX_EXPRESSION_BYTES`: UTF-8 byte length of source.
- `MAX_TOKENS`: non-`End` lexer token count.
- `MAX_NESTING_DEPTH`: nested parenthesized expressions and recursive unary `NOT` depth must never recurse without a checked bound.
- `MAX_AST_NODES`: count `Expr` nodes only. Predicate expressions count as one `Expr`; `And`, `Or`, and `Not` each count as one additional `Expr`.
- `MAX_IN_VALUES`: number of entries in an `InValues::Values` list.
- `MAX_PARAMETER_REFERENCES`: number of parameter occurrences in the source, not number of distinct names.

Check source byte length before lexing.

The lexer should enforce the token/reference limits while tokenizing. Parsing then enforces nesting, AST-node, and `IN`-value limits.

Any exceeded limit returns `expression_too_complex`.

## Frozen validation corpus

`filter-parser-validation.json` is immutable input.

Implement `tests/golden.rs` so that:

1. it loads this file;
2. expands any declared generated input deterministically;
3. calls only the public parser API;
4. renders successful ASTs into the canonical fixture JSON shape described by the file;
5. compares actual and expected values exactly;
6. compares parser errors by stable kind, span, reason, and limit where those fields are present;
7. fails loudly on any unknown fixture construct.

Do not update expected outputs from the implementation.

Do not add a "bless" or "regenerate goldens" path.

## Canonical fixture representation

The JSON corpus intentionally defines a stable external snapshot representation independent of Rust's `Debug` formatting or serde enum layout.

A test-only renderer must map the AST to that representation.

Do not shape the production AST merely to make serde emit the fixture format.

## Additional tests

The frozen corpus is the minimum acceptance bar, not the entire test suite.

Add focused unit/property-style tests for:

- every token kind;
- UTF-8 byte spans;
- case-insensitive keywords;
- keyword prefixes inside identifiers (`android`, `origin`, `inside`);
- string quote doubling;
- malformed numeric forms;
- repeated unary `NOT`;
- left-to-right collection of `AND` / `OR`;
- parentheses span widening;
- no panics for arbitrary UTF-8 input;
- all complexity boundaries at exactly limit and limit + 1 where practical.

No fuzzing framework is required, but a deterministic "never panic" corpus is encouraged.

## Forbidden shortcuts

Do not:

- hard-code validation fixture source strings;
- return prebuilt ASTs for known cases;
- special-case fixture IDs;
- inspect the validation JSON from production code;
- weaken spans to character offsets;
- parse numeric literals as `f64`;
- silently accept `!=`, `==`, `BETWEEN`, `LIKE`, `MATCH`, `CONTAINS`, `IS NULL`, arithmetic, functions, or other non-v1 syntax;
- add schema semantics;
- add a binder;
- add YQL;
- add Vespa dependencies.

## Acceptance commands

The finished standalone crate must pass:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

The frozen golden test must be part of `cargo test`.

## Definition of done

The task is complete only when:

- the crate builds independently;
- the public parser API exists;
- parsing is schema-independent;
- numbers preserve source spelling;
- precedence and parentheses match the specification;
- all tokens and AST nodes carry correct byte spans;
- `IN` remains first-class;
- all explicit complexity limits are enforced;
- malformed input returns structured errors rather than panicking;
- every frozen validation case passes exactly;
- the validation file itself is unchanged;
- fmt, clippy, and tests are green.

At completion, report:

1. files created;
2. public API;
3. complexity constants;
4. test count;
5. exact commands run and their results;
6. any ambiguity you found in the source specification.

Do not perform the later Search Kernel integration in this task.
