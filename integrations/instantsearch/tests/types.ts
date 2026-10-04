import instantsearch from 'instantsearch.js';
import { createSearchKernelInstantSearchClient } from '../src/client.js';
const client: Parameters<typeof instantsearch>[0]['searchClient'] = createSearchKernelInstantSearchClient({ endpoint: 'http://localhost/search' });
void client;
