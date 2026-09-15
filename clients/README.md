# Search Kernel clients

This directory is the package root for non-Rust language clients. They are
thin façades over the generated Portable API client and transport
implementations; they do not define Search Kernel semantics.

Client implementations are being added incrementally. The dashboard in
`searchkernel-web/` is the browser consumer and will use the shared TypeScript
client for Portable API calls.

Planned package roots are:

```text
clients/typescript/
clients/python/
clients/go/
clients/php/
clients/java/
```
