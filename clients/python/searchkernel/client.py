"""Small synchronous Python client for the canonical gRPC transport."""

from __future__ import annotations

import math
from collections.abc import Mapping, Sequence
from typing import Any

from .generated import models
from .grpc_transport import GrpcTransport, GrpcTransportError
from .wire import PortableResponseError, decode_response, encode_request


def value(item: Any) -> models.Value:
    """Convert ordinary Python values to the generated Portable API value."""
    if isinstance(item, models.Value):
        return item
    if item is None:
        return models.Value.null()
    if isinstance(item, bool):
        return models.Value.boolean(item)
    if isinstance(item, int):
        return models.Value.int64(item)
    if isinstance(item, float):
        if not math.isfinite(item):
            raise ValueError("non-finite floats are not portable")
        return models.Value.float64(item)
    if isinstance(item, str):
        return models.Value.string(item)
    if isinstance(item, bytes):
        return models.Value.bytes(item)
    if isinstance(item, Mapping):
        return models.Value.object({str(key): value(child) for key, child in item.items()})
    if isinstance(item, Sequence) and not isinstance(item, (str, bytes, bytearray)):
        return models.Value.list([value(child) for child in item])
    raise TypeError(f"unsupported value type: {type(item).__name__}")


def _document(identifier: str, fields: Mapping[str, Any]) -> models.Document:
    if not isinstance(identifier, str) or not identifier:
        raise ValueError("document id must be a non-empty string")
    return models.Document(id=identifier, fields={name: value(item) for name, item in fields.items()})


class ClientError(RuntimeError):
    """A transport or canonical Search Kernel error."""

    def __init__(self, message: str, *, error: models.Error | None = None, cause: Exception | None = None):
        super().__init__(message)
        self.error = error
        self.cause = cause


class Client:
    """Synchronous request/response client over a real gRPC channel."""

    def __init__(self, transport: GrpcTransport):
        self.transport = transport

    def execute(self, operation: str, request: Any) -> Any:
        try:
            response = self.transport.call(encode_request(operation, request))
            return decode_response(response, operation)
        except PortableResponseError as exc:
            raise ClientError(exc.error.message, error=exc.error, cause=exc) from exc
        except (GrpcTransportError, ValueError, TypeError) as exc:
            raise ClientError(str(exc), cause=exc) from exc

    def insert(self, document: models.Document) -> models.InsertResult:
        return self.execute("insert", models.InsertRequest(document=document))

    def upsert(self, document: models.Document, condition: str | None = None) -> models.UpsertResult:
        return self.execute("upsert", models.UpsertRequest(document=document, condition=condition))

    def delete(self, identifier: str, condition: str | None = None) -> models.DeleteResult:
        return self.execute("delete", models.DeleteRequest(id=identifier, condition=condition))

    def get(self, identifier: str, projection: Sequence[str] | None = None) -> models.GetResult:
        request_projection = models.FieldProjection(fields=list(projection)) if projection is not None else None
        return self.execute("get", models.GetRequest(id=identifier, projection=request_projection))

    def search(self, request: models.SearchRequest) -> models.SearchResult:
        return self.execute("search", request)

    def analyze(self, text: str, language: str | None = None, field: str | None = None) -> models.AnalyzeResult:
        target = models.AnalyzeTarget(kind="field", value=field) if field is not None else None
        return self.execute("analyze", models.AnalyzeRequest(target=target, text=text, language=language))

    def describe(self) -> models.DescribeResult:
        return self.execute("describe", models.DescribeRequest())

    def inspect_query(self, request: models.SearchRequest) -> models.InspectQueryResult:
        return self.execute("inspect_query", models.InspectQueryRequest(request=request))

    def close(self) -> None:
        self.transport.close()

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, _type, _value, _traceback) -> None:
        self.close()


class Index:
    """Convenience façade retaining the generated domain model as its API."""

    def __init__(self, client: Client):
        self.client = client

    def insert(self, identifier: str, fields: Mapping[str, Any]) -> models.InsertResult:
        return self.client.insert(_document(identifier, fields))

    def upsert(
        self, identifier: str, fields: Mapping[str, Any], condition: str | None = None
    ) -> models.UpsertResult:
        return self.client.upsert(_document(identifier, fields), condition=condition)

    def delete(self, identifier: str, condition: str | None = None) -> models.DeleteResult:
        return self.client.delete(identifier, condition=condition)

    def get(self, identifier: str, projection: Sequence[str] | None = None) -> models.GetResult:
        return self.client.get(identifier, projection)

    def search(self, request: models.SearchRequest) -> models.SearchResult:
        return self.client.search(request)

    def analyze(self, text: str, language: str | None = None, field: str | None = None) -> models.AnalyzeResult:
        return self.client.analyze(text, language, field)

    def describe(self) -> models.DescribeResult:
        return self.client.describe()

    def inspect_query(self, request: models.SearchRequest) -> models.InspectQueryResult:
        return self.client.inspect_query(request)


__all__ = ["Client", "ClientError", "Index", "value"]
