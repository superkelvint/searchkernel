import { Link, useSearchParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { useCapabilities, useSearchKernelClient } from '../../app/client-context'
import { useIndex } from '../../app/index-context'
import { clientQuery, clientQueryDefaults } from '../../app/query'
import { ErrorNotice } from '../../components/Workbench'
import { canCreateCollection, collectionPath, documentPresence, hasTextFields } from './progress'

export function FirstSearchWelcome() {
  const { capabilities } = useCapabilities(), client = useSearchKernelClient()
  const available = canCreateCollection(capabilities) && !!client.createIndex && !!client.planIndexChange
  return <section className="surface my-5 space-y-3 p-5" aria-label="First search journey">
    <h2 className="text-xl font-semibold">Start with a searchable collection</h2>
    <p>A collection, called an index in Search Kernel, holds documents you want to search: products, articles, or other records.</p>
    <ol className="grid list-inside list-decimal gap-3 sm:grid-cols-3"><li>Name your collection and define its fields.</li><li>Add a document with some searchable text.</li><li>Try your first search.</li></ol>
    <p>First, choose which information each document will contain. Search Kernel checks your fields before creating the collection.</p>
    {available ? <Link className="primary-button" to="/indexes/new?journey=first-search">Create searchable collection</Link> : <p role="status">Collection creation is unavailable from this connection. Connect to a service with index creation and field validation enabled, or select an existing index.</p>}
    <p className="muted">Existing index management and advanced configuration remain available.</p>
  </section>
}

export function CollectionNextStep({ page }: { page: 'overview' | 'documents' | 'schema' }) {
  const index = useIndex(), client = useSearchKernelClient(), { capabilities } = useCapabilities(), [params] = useSearchParams()
  const readable = capabilities?.documents.read === true && capabilities.search.basic === true
  const read = useQuery({ queryKey: ['first-search', index.id, index.generation], queryFn: clientQuery(signal => client.executeSearch(index.target ?? index.id, client.prepareSearch({ query: '', limit: 1 }), { signal })), ...clientQueryDefaults, enabled: readable })
  const presence = documentPresence(read.data), guided = params.get('journey') === 'first-search'
  if (!guided && presence === 'unknown' && !read.error && readable) return null
  if (presence === 'present' && !read.error && (!guided || page !== 'documents')) return null
  return <section className="surface mb-5 space-y-3 border-indigo-200 p-5" aria-label="Collection next step">
    <h2 className="font-semibold">{presence === 'present' ? 'Your data is ready to explore' : 'Your collection is created'}</h2>
    <p>{index.id} has {index.fields.length} defined fields. Documents use these fields to describe the information you want to find.</p>
    {capabilities?.schema.read && <Link className="text-sm font-medium text-indigo-700 underline" to={collectionPath(index, 'configuration/schema/fields')}>Review fields and advanced configuration</Link>}
    {read.error ? <><p>We could not check the collection's documents. Refresh the read before continuing; no new collection or document has been created.</p><ErrorNotice error={read.error}/></> : !hasTextFields(index) ? <><p>This collection has no declared searchable text fields. To try text search, create a collection with a searchable text field. Changing fields on an existing collection requires reindexing, which this interface does not support.</p>{canCreateCollection(capabilities) && client.createIndex && client.planIndexChange && <Link className="soft-button" to="/indexes/new?journey=first-search">Create a text-searchable collection</Link>}</> : !readable ? <p>Document browsing or search is unavailable from this connection. Your collection remains accessible through its supported administration tools.</p> : presence === 'unknown' ? <p role="status">{read.isFetching ? 'Checking documents…' : 'The document read did not establish whether the collection is empty. Browse documents to inspect the available data.'}</p> : presence === 'empty' ? <><p>Next, add a document. Put a few words in a searchable text field so you can find it in Search Lab.</p>{capabilities?.documents.write ? page === 'documents' ? <p>Choose <strong>{guided ? "Add first document" : "Create document"}</strong> to open the editor. You can also use the advanced editor for supported structured values.</p> : <Link className="primary-button" to={collectionPath(index, 'data/documents', true)}>Add your first document</Link> : <p>Adding documents is unavailable from this connection. A user with write access must add data before your first search.</p>}</> : <><p>Search Kernel returned a document from this collection. Next, search for words you added to a searchable text field.</p><Link className="primary-button" to={collectionPath(index, 'search/lab', true)}>Try your first search</Link></>}
    {readable && <button className="soft-button" disabled={read.isFetching} onClick={() => { void read.refetch() }}>{read.isFetching ? 'Checking documents…' : 'Check progress again'}</button>}
    {presence === 'unknown' && readable && !read.error && <Link className="soft-button" to={collectionPath(index, 'data/documents', true)}>Browse documents</Link>}
  </section>
}
