# Conformance architecture

```text
Idiomatic client API
        ↓
FlatBuffers portable contract
        ↓  *_to_ir
Search Kernel IR
        ↓  lower_*
Backend representation
        ↓
Execution
```

The active v0.5 corpus ends at **Search Kernel IR**. Generated FlatBuffers types are wire/view types and must not cross that boundary.

A component operation such as `facet_to_ir` or `filter_to_ir` is a compiler pass, not a backend execution operation. The backend execution trait may remain much smaller than the core IR-construction surface.
