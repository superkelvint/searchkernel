# C5 Portable Native ABI contract

This directory is normative for C5. Its checked-in verifier specification is
frozen by the verifier-authoring commit.

The production ABI is exactly:

```c
uint32_t sk_abi_version(void);
sk_status_t sk_call(sk_handle_t, const uint8_t *, size_t, sk_buffer_t *);
sk_status_t sk_close(sk_handle_t);
void sk_buffer_free(sk_buffer_t *);
```

The checked-in public header is generated from the Rust ABI declarations with
`cbindgen`; `tools/generate-searchkernel-ffi-header.sh` verifies that both the
implementation and verifier copies are identical to that generated output.

No per-operation `sk_*` functions are permitted.

## Status values

Numeric values are permanent:

| Value | Name |
|---:|---|
| 0 | `SK_STATUS_OK` |
| 1 | `SK_STATUS_INVALID_ARGUMENT` |
| 2 | `SK_STATUS_INVALID_HANDLE` |
| 3 | `SK_STATUS_INVALID_REQUEST` |
| 4 | `SK_STATUS_UNSUPPORTED_PROTOCOL` |
| 5 | `SK_STATUS_UNSUPPORTED_OPERATION` |
| 6 | `SK_STATUS_INTERNAL` |
| 7 | `SK_STATUS_PANIC` |

ABI statuses are transport/runtime failures only. Search Kernel semantic errors are encoded as `ResponseEnvelope(Error)` and `sk_call()` returns `SK_STATUS_OK`.

## Validation precedence

`sk_call()` validates in this order:

1. `out_response` must be non-null.
2. The implementation immediately writes an empty `sk_buffer_t` to
   `out_response` without inspecting its previous contents. This permits a
   caller to provide ordinary uninitialized C out-parameter storage. The
   caller must release any previously owned response before reusing that
   storage.
3. `handle` must be valid and open.
4. `(request, request_len)` must be a valid pointer/length pair. `request == NULL && request_len > 0` is `INVALID_ARGUMENT`.
5. The request envelope must be a complete verified FlatBuffer with file identifier `SKRQ`.
6. `protocol_version` must equal 1.
7. The FlatBuffers union discriminator must name a supported request payload and the union payload must be present.
8. The inner request is passed through the real `IndexContext::*_to_ir` function.

A zero-length request on a valid handle is `SK_STATUS_INVALID_REQUEST`.

## Output

After validating that `out_response` is non-null, `sk_call()` immediately
initializes all of its fields to the empty state without reading their prior
contents. On every non-OK ABI status it remains empty.

On `SK_STATUS_OK`, it contains exactly one complete `ResponseEnvelope` with file identifier `SKRS`.

Semantic errors are successful ABI calls containing `ResponseKind::Error`.

## Buffer ownership

Input request bytes are borrowed only for the duration of `sk_call()`:

- never modified;
- never freed by Search Kernel;
- no pointer into them is retained after return.

A non-empty output buffer is owned by Search Kernel and must be released with `sk_buffer_free()`.

Each non-empty output contains a process-lifetime allocation token in
`allocation_id`. The token is monotonically allocated and is never reused.
Buffer ownership is keyed by this token and the corresponding `data` pointer;
the caller-visible `len` is not used to release the allocation. Copying a
descriptor and freeing the old copy after a later allocation cannot release
the later allocation.

`sk_buffer_free()`:

- accepts `NULL` and does nothing;
- accepts an empty buffer and does nothing;
- releases a Search Kernel-owned response;
- validates the allocation ID and corresponding data pointer before releasing
  an owned response;
- sets `data = NULL`, `len = 0`, and `allocation_id = 0` before returning;
- is therefore safe to call a second time on the same cleared struct.

Passing an arbitrary foreign pointer to `sk_buffer_free()` is outside the contract.

## Handles

- `0` is permanently invalid.
- handles are opaque `uint64_t` values;
- handle values are never reused during the process lifetime;
- first successful `sk_close()` returns `OK`;
- a second close returns `INVALID_HANDLE`;
- calls beginning after close return `INVALID_HANDLE`.

Concurrent close semantics:

1. A call that has already obtained its internal `Arc` may complete normally.
2. `sk_close()` removes the handle and returns `OK`.
3. New calls after that point return `INVALID_HANDLE`.

The registry lock must not be held while executing a backend operation.

## Panic boundary

No Rust panic may unwind over the C ABI. Unexpected panic is translated to `SK_STATUS_PANIC`, with an empty output buffer.

## Production dependency boundary

Normal `searchkernel-ffi` dependencies may include `searchkernel-core`, `searchkernel-api`, and FlatBuffers infrastructure. They may not include:

- `searchkernel-vespa`
- `searchkernel-vespa-sys`
- `searchkernel-vespa-backend`
- `searchkernel-testkit`

`searchkernel-testkit` is allowed only for tests / the `ffi-test-support` feature used by this verifier.
