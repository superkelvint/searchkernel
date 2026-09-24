# Content Ownership

Every documentation topic should have one canonical home.

The documentation website may provide many paths to a page, but it should not maintain multiple competing explanations of the same Search Kernel behavior.

## Classify every page

When adding or materially restructuring documentation, identify:

1. the **primary audience**;
2. the **Diátaxis mode**;
3. the **canonical subject**;
4. whether language affects only presentation;
5. which other journeys should link to it.

Example:

```text
Filter search results

Primary audience:
    Build with SearchKernel

Mode:
    Guide

Language:
    selectable examples

Links to:
    structured-filtering concept
    filter-language reference
```

Another:

```text
Back up and restore an index

Primary audience:
    Operate SearchKernel

Mode:
    Guide

Links to:
    persistence concept
    compatibility reference
```

And:

```text
Add ergonomic client support for a capability

Primary audience:
    Contribute to SearchKernel

Mode:
    Guide

Links to:
    client architecture concept
    capability-evidence reference
```

## Prefer links over duplication

If a topic already has an authoritative page:

- link to it;
- embed or surface the relevant subsection if the site supports that cleanly;
- add a short audience-specific introduction only when necessary.

Do not copy the explanation into another audience tree.

## Audience-specific tasks may still be distinct

Shared semantics do not mean every page must serve every reader.

For example:

- "How filtering works" is a product concept.
- "Filter search results" is an application guide.
- "Add a new filter operator" is a contributor guide.

These are genuinely different documentation tasks and may have separate pages.

## Specifications versus documentation

The documentation website teaches and explains the supported product. It does not redefine product semantics.

Normative behavior remains owned by the appropriate Search Kernel contracts, specifications, code, and verified capability evidence.

If documentation and a normative authority disagree, fix the documentation or deliberately change the product contract. Do not create documentation-only semantics.
