# Diátaxis

Audience and Diátaxis solve different problems.

Audience answers:

> Who is the reader, and what are they trying to accomplish with Search Kernel?

Diátaxis answers:

> What kind of help do they need right now?

Each documentation audience may contain four modes:

```text
Tutorial
    learning-oriented

Guide
    task-oriented

Concept
    understanding-oriented

Reference
    information-oriented
```

Public labels may use more natural wording. The discipline matters more than mechanically exposing the words "Tutorial", "How-to", "Explanation", and "Reference" everywhere.

## Tutorials

Tutorials are linear, opinionated learning experiences.

They should optimize for a successful outcome and a useful mental model rather than completeness.

Examples:

- build your first searchable index;
- deploy Search Kernel for the first time;
- make a first contributor change.

A tutorial should not become an exhaustive reference page.

## Guides

Guides answer concrete task questions.

Examples:

- filter search results;
- configure TCP access;
- back up and restore an index;
- add ergonomic support for a new client capability.

A guide should contain the explanation needed to complete the task and link to deeper concepts or reference instead of absorbing them.

## Concepts

Concept pages explain models, relationships, architecture, and reasoning.

Examples:

- full-text search versus structured filtering;
- facets are grouping;
- Search Kernel and Vespa;
- persistence and reopen;
- why non-Rust clients use `searchkerneld`;
- portable semantics;
- the verification architecture.

A concept page should not masquerade as a step-by-step guide.

## Reference

Reference is concise, complete, and mechanically trustworthy where practical.

Examples:

- SearchBuilder API;
- filter syntax;
- error codes;
- `searchkerneld` configuration;
- CLI options;
- capability mappings;
- verification targets.

## Cross-linking modes

The modes should reinforce each other:

```text
tutorial
   ↓
guide
 ↙   ↘
concept  reference
```

A guide may link to a concept for deeper understanding and to reference for complete details.

A reference page should link to the guide that shows the capability in a realistic task.

Do not duplicate content merely to keep every mode self-contained.
