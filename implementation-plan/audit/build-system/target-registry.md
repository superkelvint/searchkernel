# Canonical build target registry

`build/targets.json` is the GitHub-readable authority for the finite public
`./dev test` and `./dev verify` vocabularies. `./dev` and the Vespa verifier
router consume it rather than maintaining independent discovery lists.

The registry also records the required public command vocabulary and lightweight
change-area guidance. A command marked `planned_*` is deliberately visible but
fails explicitly until its roadmap milestone implements it; it is not aliased
to a different command merely to appear complete.

Aliases are compatibility inputs only. Discovery prints canonical targets first,
and dispatch resolves aliases back to those canonical names before execution.
