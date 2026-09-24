# Contribute to SearchKernel

## Reader intent

> I am changing Search Kernel itself.

This journey is for contributors and maintainers working on the Search Kernel implementation, tooling, tests, verification, packaging, clients, daemon, or Vespa integration.

## Typical subjects

- development setup;
- reading the codebase;
- architecture;
- repository organization;
- making a change;
- adding a capability;
- portable API and Search Kernel IR;
- Rust product API;
- client facade implementation;
- adding a client language;
- `searchkerneld`;
- transport;
- Vespa integration;
- native C++;
- testing and verification;
- CI;
- generated artifacts;
- updating Vespa;
- release engineering;
- documentation infrastructure.

## Client usage versus client implementation

This distinction is mandatory.

Using a TypeScript, PHP, Python, Rust, or other Search Kernel client belongs under:

```text
Instant Search
or
Build with SearchKernel
```

Implementing, extending, or maintaining one of those clients belongs here.

Do not create a TypeScript, PHP, Python, or Rust documentation universe merely because both users and maintainers interact with that package.

## Vespa material

Developer-facing explanations of Vespa-backed Search Kernel behavior belong under **Build with SearchKernel**.

Implementation material such as lowering, native boundaries, engine integration, fidelity verification, and adding a new Vespa capability belongs here.

This gives Vespa material two clear homes according to reader intent:

```text
"What does this Vespa capability mean to a Search Kernel user?"
    → Build with SearchKernel

"How does Search Kernel implement or verify this Vespa capability?"
    → Contribute to SearchKernel
```

## Diátaxis within this audience

Typical shapes include:

- **Start contributing:** tutorial/onboarding path.
- **Development guides:** task-oriented change recipes.
- **Architecture:** explanation and system mental models.
- **Developer reference:** commands, verifier targets, generated artifacts, repository rules.

The Diátaxis rules in [../diataxis.md](../diataxis.md) apply.
