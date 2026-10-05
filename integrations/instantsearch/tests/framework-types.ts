// The pinned React declarations use bundler-style extensionless re-exports.
import type { ComponentProps } from 'react';
import { InstantSearch } from 'react-instantsearch';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';
const client: ComponentProps<typeof InstantSearch>['searchClient'] = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search'});
void client;
