# Search Kernel portable FFI safety

The portable C ABI has deliberately narrow concurrency guarantees. These are
caller obligations, not additional synchronization performed by the ABI.

## Handles and close

`sk_close(handle)` removes the handle from the registry and prevents new calls
from starting. It is not a completion barrier: a call that already acquired
the handle's internal strong reference may continue and complete normally
after `sk_close` returns. A caller that requires all work to finish must
coordinate that quiescence itself before closing the handle.

After close returns, future calls using that handle fail with
`SK_STATUS_INVALID_HANDLE`. Handle destruction is independent of the lifetime
of an in-flight call.

## Request and response storage

Request bytes are borrowed for the duration of `sk_call`. The request storage
must remain readable, unchanged, and mapped until `sk_call` returns. The
caller must not concurrently write, free, unmap, or reuse it.

The three output arguments passed to `sk_call` are caller-owned writable
storage. They must not be concurrently read, written, or reused while
`sk_call` is using them. On success, `out_data` and `out_len` describe the
response bytes and `out_allocation_id` is the ownership token.

The caller releases the response with `sk_buffer_free(out_data,
out_allocation_id)`. The token and data-pointer checks ensure that only the
first valid pointer/token pair releases the underlying response allocation. A
stale copied pair cannot release a later allocation, even if the allocator
reuses the same address. `sk_buffer_free` does not modify the caller's length
variable; the caller must stop using the bytes after freeing them.
