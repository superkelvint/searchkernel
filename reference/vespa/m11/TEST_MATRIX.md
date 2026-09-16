# M11 test matrix

| Area | Required coverage | Result |
| --- | --- | --- |
| Lifecycle | 1,000 open/drop cycles; failed open then valid open | no crash, leak, or poisoned process |
| Reopen | 250 mutate/close/reopen cycles | exact logical state survives |
| Ownership | shared owners, worker joins, final owner close | destruction follows last safe owner |
| Inputs | invalid identifiers, fields, types, ranges, YQL, UTF-8, lengths | controlled error |
| Structure | deep/wide query trees and large text/term inputs | success within limits or controlled error |
| Failures | before/after mutation, search, result copy, open, destruction | cleanup and documented side effects |
| M8 results | sorted hits, offsets, summaries, finite scores, result faults | controlled conversion and ownership |
| M9 state | grouping, facets, filtered/sorted attributes, attribute failpoints | native state remains coherent |
| M10 ranking | rank profiles, query features, rank properties, match features | finite deterministic scores/features |
| Adapter | real `searchkernel-ir-adapter` CRUD, projection, filter, sort, facets | no semantic fallback |
| Production C5 | `sk_create`, `sk_open`, `sk_call`, buffer release, `sk_close` | stable status and ownership |
| Exceptions | `std::exception`, bad allocation, unknown exception | stable native status, no ABI unwind |
| Concurrency | concurrent searches, readers/writers, disjoint writers | no impossible state or corruption |
| Stress | fixed-seed state trace and mixed worker workload | exact quiescent verification |
| Resources | heap, FDs, threads, PSS/RSS, read-only disk growth | frozen guardrails |
| Sanitizers | ASan/LSan/UBSan and separate TSan lanes | no Search Kernel finding |
