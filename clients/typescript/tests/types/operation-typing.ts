import { Client } from "../../src/client/index.js";
import { InsertRequestT } from "../../src/semantic.js";
import { SearchRequestT } from "../../src/semantic.js";
import type { GrpcWebTransport } from "../../src/transport/grpc-web.js";

const client = new Client({ execute: async () => new Uint8Array() } as Pick<GrpcWebTransport, "execute">);
void client.execute("insert", new InsertRequestT());
// @ts-expect-error an insert discriminator must never carry a SearchRequest table
void client.execute("insert", new SearchRequestT());
