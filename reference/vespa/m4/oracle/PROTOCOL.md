# M4 direct C++ oracle protocol

The direct C++ oracle is a long-running stdin/stdout process. It owns one real
embedded Proton instance for the duration of a scenario.

Input lines are tab-separated ASCII. Raw strings and analysis payloads are UTF-8
encoded as lowercase hex so tabs/newlines/non-ASCII never affect framing.

The C++ oracle deliberately does **not** parse M3 JSON and does not contain any
linguistic behavior. `run_scenario.py` mechanically converts each M3 term into:

```text
position,byte_start,byte_len,term_hex
```

and joins records with `;`. That compact string is then hex-encoded as the final
protocol field. The original raw document text is sent separately.

## Commands

### PUT / UPDATE

```text
PUT<TAB>native_document_id<TAB>raw_text_hex<TAB>analysis_spec_hex
UPDATE<TAB>native_document_id<TAB>raw_text_hex<TAB>analysis_spec_hex
```

The oracle constructs a real Vespa `StringFieldValue` containing the original
raw text and a `linguistics` span tree. Each analyzed term becomes a real `TERM`
annotation attached to its M3 source span. If the analyzed term differs from the
source substring, the annotation carries a `StringFieldValue(term)` value,
matching Vespa's linguistic annotation convention.

Reply:

```text
OK
```

### DELETE

```text
DELETE<TAB>native_document_id
```

Reply `OK`.

### TERM / PHRASE

```text
TERM<TAB>search_id<TAB>rank_profile<TAB>analysis_spec_hex
PHRASE<TAB>search_id<TAB>rank_profile<TAB>analysis_spec_hex
```

The oracle constructs a real Vespa query tree using
`QueryBuilder<proton::matching::ProtonNodeTypes>` and runs
`DocumentDB::match`.

Reply exactly one line:

```text
RESULT<TAB>search_id<TAB>native_id,score;native_id,score;...
```

Hit order is Vespa hit order. Scores are emitted with `max_digits10` from the
real `SearchReply::Hit::metric` (`search::HitRank`, a `double`). The canonical
scenario strips scores from checkpoints that do not request them.

### QUIT

```text
QUIT
```

Reply `OK` and exit zero.

## Required path

The oracle process directly instantiates/uses pinned Vespa C++ classes. It must
not link or call Search Kernel Rust or `native/vespa` implementation code.
