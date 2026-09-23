# Instant Search

## Reader intent

> I want to add an excellent search experience to my application quickly.

Instant Search is the most opinionated and outcome-oriented Search Kernel documentation journey.

Its job is to get a developer from application data to a useful search experience with the fewest unnecessary decisions.

## Typical subjects

- first searchable index;
- product search;
- site or CMS search;
- instant-search UI;
- indexing documents;
- basic text search;
- filters;
- facets;
- sorting;
- pagination;
- highlighting;
- common relevance choices;
- common deployment path where required to complete the experience.

## Documentation style

Instant Search documentation should:

- lead with successful defaults;
- prefer end-to-end outcomes over exhaustive capability coverage;
- keep the happy path short;
- use believable application examples;
- explain only the concepts required for the task;
- provide clear links into **Build with SearchKernel** when the reader needs more control.

A first-time reader should not need to understand FlatBuffers, Search Kernel IR, Proton, native bindings, transport framing, or Search Kernel's internal architecture.

## Boundary with Build with SearchKernel

Instant Search is **not** a second or simplified semantic model.

It is an opinionated teaching and product journey over the same Search Kernel API and semantics documented by Build with SearchKernel.

The expected progression is:

```text
Instant Search
    ↓
successful default experience
    ↓
"I need more control"
    ↓
Build with SearchKernel
```

Do not duplicate deeper product documentation under Instant Search merely to keep a reader inside this section. Link to the canonical guide, concept, or reference page instead.

## Diátaxis within this audience

Typical shapes include:

- **Get started:** build a first search experience.
- **Guides:** add filtering, facets, sorting, highlighting, or another common capability.
- **Concepts:** only the mental models needed to make good application choices.
- **Reference:** concise entry points for common configuration or UI-facing options.

The Diátaxis rules in [../diataxis.md](../diataxis.md) apply.
