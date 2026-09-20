# Phase 8 grouping compiler source trace

The grouping compiler is pinned to Vespa revision
`2300ef0412bdb76f227ae3b497c8ca5e73c5da4e` and follows:

```text
upstream/vespa/container-search/src/main/java/com/yahoo/search/grouping/vespa/RequestBuilder.java
```

The relevant `RequestBuilder` sequence is `processRequestNode`: non-`Each`
nodes call `resolveOutput` on the current leaf `Group`, while `Each` first
calls `resolveEach`. `resolveEach` consumes pending `groupBy` state into a
`GroupingLevel`; its later `resolveOutput` therefore attaches outputs inside
that `Each` to the newly materialized group prototype.

`build` clones grouping state for child branches and can emit multiple backend
`Grouping` objects. The adapter's native plan is intentionally linear, so
parallel branches are rejected instead of being merged or silently relocated.
This is also why an output on a parent `all` node is rejected and why `all`
cannot substitute for the required `each` materialization.
