"""Search Kernel's version-free Python client façade."""

from .client import Client, ClientError, Index, SearchKernelError, Tensor, uint64, value
from .grpc_transport import GrpcTransport, GrpcTransportError
from .ergonomic import (
    Boolean, Coverage, CoverageDegraded, Float, Group, Hit, HnswIndexConfig, Integer, Keyword,
    PatchBuilder, RankProfile, ResultContinuation, ResultGroup, ResultIssue, ResultLimits,
    SearchBuilder, SearchResults, SearchTiming, TensorField, Text, UpdateBuilder,
    PositionField, StructField, ArrayField,
    YqlBuilder, agg, asc, desc, g, gf, q,
)

__all__ = [
    "Client", "ClientError", "SearchKernelError", "Index", "GrpcTransport",
    "GrpcTransportError", "Boolean", "Coverage", "CoverageDegraded", "Float",
    "Group", "Hit", "HnswIndexConfig", "Integer", "Keyword", "PatchBuilder", "RankProfile",
    "ResultContinuation", "ResultGroup", "ResultIssue", "ResultLimits", "SearchBuilder",
    "SearchResults", "SearchTiming", "Tensor", "TensorField", "PositionField", "StructField", "ArrayField",
    "Text", "UpdateBuilder", "YqlBuilder", "agg", "asc", "desc", "g", "gf",
    "q", "uint64", "value",
]
