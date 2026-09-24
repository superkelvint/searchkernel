# Search Kernel Documentation Framework

**Status:** Authoritative documentation framework  
**Scope:** The Search Kernel documentation website and all documentation intended to appear within it

This framework defines the durable information architecture for Search Kernel documentation.

It answers three independent questions:

```text
WHO IS THE READER?
        ↓
documentation audience

WHAT KIND OF HELP DO THEY NEED?
        ↓
Diátaxis mode

WHICH LANGUAGE ARE THEY USING?
        ↓
presentation preference
```

These dimensions must remain separate.

Search Kernel has **one documentation product**. It must not fragment into separate documentation universes for Python, TypeScript, PHP, Rust, `searchkerneld`, Vespa integration, or contributors.

## Primary audiences

The documentation website has four primary user journeys:

1. [Instant Search](audiences/instant-search.md) — build a high-quality search experience quickly.
2. [Build with SearchKernel](audiences/build-with-searchkernel.md) — use and understand the full Search Kernel product surface.
3. [Operate SearchKernel](audiences/operate-searchkernel.md) — deploy, run, maintain, and troubleshoot Search Kernel.
4. [Contribute to SearchKernel](audiences/contribute.md) — change Search Kernel itself.

These are the website's primary front doors. The exact public wording may evolve, but the distinction between the four reader intents is part of the documentation architecture.

## Cross-cutting rules

- [Diátaxis](diataxis.md) defines how tutorials, guides, concepts, and reference work inside each audience journey.
- [Language model](language-model.md) defines the global language preference and prohibits separate per-language documentation trees.
- [Navigation](navigation.md) defines how audience landing pages and canonical content interact.
- [Content ownership](content-ownership.md) defines canonical homes, cross-linking, and duplication rules.
- [Governance](governance.md) defines how new documentation is classified and reviewed.

## Relationship to the implementation plan

This framework owns **audience, information architecture, content ownership, and documentation-governance rules**.

The implementation plan at
`implementation-plan/Developer Documentation — Unified Diátaxis and Generated Examples Plan.md`
owns implementation details such as:

- persistent language selection;
- semantic example IDs;
- executable example extraction and verification;
- generated API reference;
- capability evidence integration;
- documentation build verification;
- documentation search.

The implementation plan must conform to this framework. It must not introduce an alternative site hierarchy.

## Guiding principle

A reader should be able to answer two questions immediately:

> Which path is for me?

and:

> What should I read next?

Everything in this framework exists to make those answers obvious while preserving one coherent Search Kernel product model.
