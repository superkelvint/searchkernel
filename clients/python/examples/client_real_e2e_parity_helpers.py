from __future__ import annotations

from typing import Any

from searchkernel import agg, g


def aggregation_expression(spec: dict[str, Any]):
    kind = spec["kind"]
    if kind == "count":
        return agg.count()
    if kind == "sum":
        return agg.sum(spec["field"])
    if kind == "avg":
        return agg.avg(spec["field"])
    raise ValueError(f"unsupported parity aggregation kind: {kind}")


def aggregation_output(spec: dict[str, Any]):
    if spec["kind"] == "alias":
        return g.alias_ref(spec["name"]).as_(spec["output_name"])
    return aggregation_expression(spec).as_(spec["name"])
