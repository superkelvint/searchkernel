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

The `sk_buffer_t` descriptor passed as `out_response` is caller-owned writable
storage. It must not be concurrently read, written, reused, or passed to
`sk_buffer_free` while `sk_call` is using it.

Likewise, one `sk_buffer_t` descriptor must not be passed concurrently to
multiple `sk_buffer_free` calls. Independent copies of a descriptor may be
freed independently; the allocation token and data-pointer checks ensure that
only the first valid copy releases the underlying response allocation. A
stale copy cannot release a later allocation, even if the allocator reuses the
same address.

The caller must ensure that no thread accesses a descriptor while another
thread is mutating it. This includes the `data`, `len`, and `allocation_id`
fields being cleared by `sk_buffer_free`.
