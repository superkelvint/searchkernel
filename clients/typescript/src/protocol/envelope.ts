/** Version-free client operations delegated to the private wire codec. */
export {
  expectedResponseType,
  packObject,
  packRequest,
  ProtocolError,
  PROTOCOL_VERSION,
  REQUEST_IDENTIFIER,
  RESPONSE_IDENTIFIER,
  unpackResponse,
} from "../_wire/v1/codec.js";
export type { DecodedResponse, Operation, OperationRequestMap } from "../_wire/v1/codec.js";
