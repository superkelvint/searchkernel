import { Client } from "../../src/client/index.js";
import { FieldDefinitionT, IndexDefinitionT } from "../../src/index.js";
import { InsertRequestT, Int64ValueT, SearchRequestT, ValueKind, ValueT } from "../../src/semantic.js";
import type { GrpcWebTransport } from "../../src/transport/grpc-web.js";

const client = new Client({ execute: async () => new Uint8Array() } as Pick<GrpcWebTransport, "execute">);
void client.execute("insert", new InsertRequestT());
// @ts-expect-error an insert discriminator must never carry a SearchRequest table
void client.execute("insert", new SearchRequestT());


const fieldDefinition = new FieldDefinitionT("title");
void new IndexDefinitionT("products", [fieldDefinition]);
void new ValueT(ValueKind.Int64Value, new Int64ValueT(1n));
// @ts-expect-error semantic constructors retain their field types
void new FieldDefinitionT(123);
// @ts-expect-error semantic constructors reject extra positional arguments
void new IndexDefinitionT("products", [], null, [], null, [], [], [], [], [], [], [], [], [], [], "extra");
