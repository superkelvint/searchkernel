# Operate SearchKernel

## Reader intent

> I need to deploy, run, maintain, upgrade, secure, or troubleshoot Search Kernel.

This journey serves operators and application developers responsible for production runtime behavior.

Its primary runtime subject is `searchkerneld`, together with persistent indexes and deployment concerns.

## Typical subjects

- installation;
- starting and stopping `searchkerneld`;
- configuration;
- Unix-domain sockets;
- TCP or network deployment;
- index locations;
- permissions;
- resource sizing;
- process lifecycle;
- health and readiness;
- logging;
- metrics and observability;
- persistence;
- crash recovery;
- backup and restore;
- upgrades;
- compatibility;
- restricted or read-only serving;
- performance;
- troubleshooting.

## Boundary with product documentation

Operational documentation must distinguish product semantics from runtime behavior.

For example:

```text
meaning of a filter
    → Build with SearchKernel

configure, monitor, secure, and upgrade the daemon executing it
    → Operate SearchKernel
```

Likewise, a conceptual explanation of Search Kernel persistence may be canonical product material, while "Back up and restore an index" is an operational guide.

Cross-link rather than duplicate.

## Maintainer versus operator material

A person operating `searchkerneld` is not automatically a Search Kernel contributor.

User-facing deployment, configuration, runtime, and troubleshooting material belongs here.

Information about changing the daemon implementation, transport architecture, verification, or source code belongs under **Contribute to SearchKernel**.

## Diátaxis within this audience

Typical shapes include:

- **Get started:** first local or production deployment.
- **Guides:** configure, deploy, back up, restore, upgrade, troubleshoot.
- **Concepts:** lifecycle, persistence, process ownership, runtime architecture.
- **Reference:** CLI, configuration, environment variables, limits, health behavior.

The Diátaxis rules in [../diataxis.md](../diataxis.md) apply.
