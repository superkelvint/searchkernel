# Build with SearchKernel

## Reader intent

> I am building an application with Search Kernel and need to understand and control its search capabilities.

This is the primary product documentation for developers using Search Kernel beyond the most opinionated Instant Search path.

## Typical subjects

- indexes and documents;
- schemas and field types;
- document operations;
- text search;
- structured filtering;
- typed queries;
- sorting;
- pagination;
- projection;
- highlighting;
- facets;
- grouping and aggregations;
- ranking;
- query inputs;
- vectors and tensors;
- YQL;
- result trees;
- errors;
- request inspection;
- advanced escape hatches.

This journey owns the developer-facing explanation of the Search Kernel product model.

## Search Kernel and Vespa

Vespa is not a separate top-level documentation audience.

Developer-facing Vespa material belongs primarily in this journey, especially in Concepts and Reference, when it helps a developer understand or use Search Kernel correctly.

Examples include:

- what Search Kernel owns;
- what Vespa owns;
- how Search Kernel concepts map to Vespa concepts;
- capabilities inherited from Vespa;
- ranking profiles;
- grouping;
- YQL;
- vectors and tensors;
- raw or advanced escape hatches;
- meaningful differences from standalone Vespa.

The desired progression is:

```text
ordinary Search Kernel usage
        ↓
advanced Search Kernel concepts
        ↓
underlying Vespa capability where useful
```

A developer should not have to learn Vespa before using Search Kernel. An expert should be able to discover exactly what Vespa capability is underneath a Search Kernel concept and how much of it Search Kernel exposes.

## Boundary with Instant Search

Instant Search answers:

> How do I get a strong search experience working quickly?

Build with SearchKernel answers:

> How does this capability work, and how do I control it?

The same canonical page may be linked from both journeys when appropriate. Do not create an "Instant Search version" and an "advanced version" of the same semantic explanation unless the tasks are genuinely different.

## Language handling

This journey is language-neutral in structure and semantics.

Code examples, API signatures, installation commands, and genuine host-language differences follow the global language model in [../language-model.md](../language-model.md).

There are no separate TypeScript, PHP, Python, Rust, or other documentation trees.
