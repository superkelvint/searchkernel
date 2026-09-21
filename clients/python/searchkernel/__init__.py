"""Search Kernel's Python façade and canonical generated V2 client."""

from .client import Client, ClientError, Index, SearchKernelError, Tensor, value
from .grpc_transport import GrpcTransport, GrpcTransportError
from .ergonomic import (
    Boolean, Coverage, CoverageDegraded, Float, Group, Hit, HnswIndexConfig, Integer, Keyword,
    PatchBuilder, RankProfile, ResultGroup, SearchBuilder, SearchResults,
    TensorField, Text, UpdateBuilder, YqlBuilder, agg, asc, desc, g, gf, q,
)

__all__ = [
    "Client", "ClientError", "SearchKernelError", "Index", "GrpcTransport",
    "GrpcTransportError", "Boolean", "Coverage", "CoverageDegraded", "Float",
    "Group", "Hit", "HnswIndexConfig", "Integer", "Keyword", "PatchBuilder", "RankProfile",
    "ResultGroup", "SearchBuilder", "SearchResults", "Tensor", "TensorField",
    "Text", "UpdateBuilder", "YqlBuilder", "agg", "asc", "desc", "g", "gf",
    "q", "value",
]
