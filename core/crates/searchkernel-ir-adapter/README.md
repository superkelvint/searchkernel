# `searchkernel-ir-adapter`

This crate is the only integration boundary that may depend on both
`searchkernel-ir` and `searchkernel-vespa-engine`.

It lowers owned Search Kernel IR into the safe low-level Vespa engine API. The
portable core and the Vespa engine wrapper must remain independently usable.
