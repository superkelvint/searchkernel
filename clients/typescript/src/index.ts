export { Client, Daemon, Index, SearchKernelError, type DocumentInput, type OperationResponseMap } from "./client/index.js";
export {
  Tensor,
  value,
  int64,
  uint64,
  float64,
  fromValue,
  ValueDecodeError,
  type Int64,
  type ValueInput,
  type DecodedTensor,
  type DecodedTensorAddressEntry,
  type DecodedSparseTensorCell,
  type DecodedMixedTensorBlock,
} from "./client/values.js";
export {
  Coverage,
  CoverageDegraded,
  Hit,
  ResultGroup,
  SearchResults,
  ResultDecodeError,
  type FacetBucket,
  type ResultChild,
  type ResultContinuation,
  type ResultLimits,
  type TotalHitsRelationName,
} from "./client/results.js";
export {
  GrpcWebTransport,
  TransportError,
  type ExecuteOptions,
  type GeneratedRpcClient,
} from "./transport/grpc-web.js";
export {
  SearchBuilder,
  YqlBuilder,
  PatchBuilder,
  UpdateBuilder,
  type FacetOptions,
} from "./ergonomic/builders.js";
export {
  Group,
  GroupingExpr,
  GroupingOrder,
  GroupingOutput,
  agg,
  asc,
  desc,
  g,
  gf,
  type FacetSort,
  type GroupingInput,
} from "./ergonomic/grouping.js";
export { q, type TypedQuery } from "./ergonomic/query.js";

export { Text, Keyword, Integer, Float, Boolean, TensorField, PositionField, ArrayOfStructField, RankProfile, type FieldOptions } from "./ergonomic/schema.js";
export { AttributeConfigT, OptionalBoolT, IndexDefinitionT, RankPhaseDefinitionT, RankProfileDefinitionT } from "./semantic.js";
export { FieldDefinitionT } from "./semantic.js";
export { DataTypeKind, PrimitiveDataTypeKind, PrimitiveDataTypeT } from "./semantic.js";
export { ProtocolError } from "./protocol/envelope.js";
export type { ClientTransport } from "./client/index.js";
