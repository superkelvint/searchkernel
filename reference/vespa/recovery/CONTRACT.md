# Cross-boundary persistence recovery contract

This lane verifies that durable state survives an actual process crash after
the state has crossed the product lifecycle and C5 boundaries. It is separate
from M5 because M5 deliberately stops at the low-level `Engine` wrapper.

The two exercised paths are:

```text
IndexRuntime::create/open -> schema compiler -> IR adapter -> native engine
sk_create/sk_open/sk_call/sk_close -> IndexRuntime/IR -> native engine
```

Each case uses a setup process, a crash worker, and a fresh verifier process.
The crash worker opens the already-created index and aborts immediately before
or immediately after one operation. It never calls `create` during a crash
case.

## Matrix

The frozen matrix is:

```text
insert  before, after
update  before, after
delete  before, after
close   before, after
```

After reopening, the verifier checks real search and get results. Since the
embedded Vespa transaction log uses its configured non-fsync commit behavior,
the post-crash mutation result is bounded as follows:

```text
insert: absent or present
update: old value or new value
delete: present or absent
close: unchanged baseline state
```

Only these operation-local alternatives are accepted. The verifier never
asserts a stronger power-loss guarantee or an atomic multi-operation result.

## Identity and no-fresh-index rule

The setup process snapshots the product-managed manifest and compiled schema
files. Every verifier process must observe byte-identical identity files after
reopen, and `describe` must still report the expected schema through the
boundary under test.

The original seed documents must also remain observable. An `open` operation
against a nonexistent path must fail and must not create the path. These checks
make an accidental fresh-index fallback observable even when the crashed
operation itself was not durable.

The repeated scenario runs the same boundary sequence for 20 cycles on one
directory, using unique insert/delete targets and a stable update target. The
complete eight-case matrix is repeated three times on isolated directories.

The lane uses the pinned Vespa revision and the production native SDK. It does
not modify the M5 compatibility fixture, the M11 verifier, or the public Rust
and C ABI.
