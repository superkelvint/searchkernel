# Documentation Governance

This file defines the review questions for adding or materially changing documentation website content.

## Placement checklist

Before adding a page, answer:

### 1. Who is the primary reader?

Choose one primary audience:

- Instant Search;
- Build with SearchKernel;
- Operate SearchKernel;
- Contribute to SearchKernel.

A page may be linked from other journeys, but it should normally have one canonical home.

### 2. What kind of documentation is it?

Choose the dominant Diátaxis mode:

- tutorial;
- guide;
- concept;
- reference.

Avoid mixing modes unnecessarily.

### 3. Does the subject already have a canonical page?

Prefer:

- cross-linking;
- a short audience-specific introduction;
- reusable verified examples;

over duplicated conceptual prose.

### 4. Is language variation presentation-only?

For end-user product documentation, language normally changes examples, signatures, installation commands, and genuine host-language callouts — not the page hierarchy or semantics.

Do not add a per-language documentation tree.

### 5. Is the level of detail appropriate to the audience?

Ask whether the page introduces implementation detail too early.

For example:

- Instant Search should not require IR or FlatBuffers knowledge.
- Build with SearchKernel may explain Vespa concepts where useful to the product mental model.
- Operate SearchKernel should focus on runtime behavior rather than source internals.
- Contribute may explain implementation architecture and verification in depth.

### 6. What should the reader do next?

Every non-reference page should provide a natural next step:

- a more advanced guide;
- a deeper concept;
- the relevant reference;
- the next audience journey.

## Relationship to specifications

Documentation explains and teaches supported behavior.

It must not invent or silently override:

- portable API semantics;
- Search Kernel IR semantics;
- client API behavior;
- filter-expression semantics;
- runtime contracts;
- capability availability;
- Vespa behavior.

Capability claims should be grounded in canonical product evidence.

## Relationship to the unified developer-documentation plan

The unified Diátaxis/generated-examples plan is the implementation authority for:

- executable documentation examples;
- global language selection behavior;
- semantic example IDs;
- generated API reference;
- capability-aware rendering;
- documentation verification;
- documentation search.

This framework is the information-architecture authority.

If future implementation work would create a second documentation hierarchy, a separate per-language docs universe, or conflicting content ownership, change the implementation rather than bypassing this framework.

## Quality test

A reader should be able to determine quickly:

```text
Which path is for me?
What kind of page am I reading?
What should I read next?
```

If the information architecture makes those answers ambiguous, treat that as a documentation design defect.
