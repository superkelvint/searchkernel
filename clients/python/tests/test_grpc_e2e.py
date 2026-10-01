"""Python generated-object API -> gRPC -> persisted native backend."""

from __future__ import annotations

import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Client, ClientError, GrpcTransport, Index, q  # noqa: E402
from searchkernel.client import value  # noqa: E402
from searchkernel.protocol import pack_object, pack_request  # noqa: E402
from SearchKernel.V2.AnalyzeFieldTarget import AnalyzeFieldTargetT  # noqa: E402
from SearchKernel.V2.AnalyzeRequest import AnalyzeRequestT  # noqa: E402
from SearchKernel.V2.AnalyzeTarget import AnalyzeTarget  # noqa: E402
from SearchKernel.V2.DataType import DataTypeT  # noqa: E402
from SearchKernel.V2.DataTypeKind import DataTypeKind  # noqa: E402
from SearchKernel.V2.FieldDefinition import FieldDefinitionT  # noqa: E402
from SearchKernel.V2.ArrayDataType import ArrayDataTypeT  # noqa: E402
from SearchKernel.V2.StructDataType import StructDataTypeT  # noqa: E402
from SearchKernel.V2.StructFieldDefinition import StructFieldDefinitionT  # noqa: E402
from SearchKernel.V2.FilterClause import FilterClauseT  # noqa: E402
from SearchKernel.V2.FilterRequest import FilterRequestT  # noqa: E402
from SearchKernel.V2.IndexDefinition import IndexDefinitionT  # noqa: E402
from SearchKernel.V2.MatchingConfig import MatchingConfigT  # noqa: E402
from SearchKernel.V2.MatchingMode import MatchingMode  # noqa: E402
from SearchKernel.V2.OptionalBool import OptionalBoolT  # noqa: E402
from SearchKernel.V2.PrimitiveDataType import PrimitiveDataTypeT  # noqa: E402
from SearchKernel.V2.LiteralQueryInput import LiteralQueryInputT  # noqa: E402
from SearchKernel.V2.QueryInput import QueryInputT  # noqa: E402
from SearchKernel.V2.QueryInputKind import QueryInputKind  # noqa: E402
from SearchKernel.V2.RankPhaseDefinition import RankPhaseDefinitionT  # noqa: E402
from SearchKernel.V2.RankProfileDefinition import RankProfileDefinitionT  # noqa: E402
from SearchKernel.V2.RankingRequest import RankingRequestT  # noqa: E402
from SearchKernel.V2.SearchKernelTextQuery import SearchKernelTextQueryT  # noqa: E402
from SearchKernel.V2.QuerySource import QuerySourceT  # noqa: E402
from SearchKernel.V2.QuerySourceKind import QuerySourceKind  # noqa: E402
from SearchKernel.V2.SearchRequest import SearchRequestT  # noqa: E402
from SearchKernel.V2.MatchAllQuery import MatchAllQueryT  # noqa: E402
from SearchKernel.V2.OptionalUInt32 import OptionalUInt32T  # noqa: E402
from SearchKernel.V2.StringEntry import StringEntryT  # noqa: E402
from SearchKernel.V2.PositionDataType import PositionDataTypeT  # noqa: E402
from SearchKernel.V2.ValueEntry import ValueEntryT  # noqa: E402


ROOT = Path(__file__).resolve().parents[3]
DAEMON = Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target" / "host"))) / "debug" / "searchkerneld"
SDK_LIB = ROOT / "core" / "native" / "sdk" / "linux-x86_64" / "lib"
VESPA_DEPS_LIB = ROOT / "vespa-deps" / "lib64"


def _field(name: str, primitive: int, *, matching: int = MatchingMode.Unspecified) -> FieldDefinitionT:
    return FieldDefinitionT(
        name=name,
        dataType=DataTypeT(
            kindType=DataTypeKind.PrimitiveDataType,
            kind=PrimitiveDataTypeT(kind=primitive),
        ),
        stored=OptionalBoolT(True),
        searchable=OptionalBoolT(primitive == 1 and matching == MatchingMode.Text),
        filterable=OptionalBoolT(primitive != 1 or matching == MatchingMode.Exact),
        sortable=OptionalBoolT(primitive != 1 or matching == MatchingMode.Exact),
        facetable=OptionalBoolT(primitive != 1 or matching == MatchingMode.Exact),
        matching=MatchingConfigT(mode=matching) if primitive == 1 else None,
    )


class RealGrpcPythonE2ETest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(["cargo", "build", "-p", "searchkerneld"], cwd=ROOT, check=True)

    def setUp(self):
        self.tempdir = tempfile.TemporaryDirectory(prefix="searchkernel-python-e2e-")
        root = Path(self.tempdir.name)
        self.index_path = root / "index"
        self.target = "python-e2e"
        self.definition = IndexDefinitionT(
            name="python_e2e",
            fields=[
                _field("title", 1, matching=MatchingMode.Text),
                _field("category", 1, matching=MatchingMode.Exact),
                _field("price", 5),
            ],
        )
        self.process = None
        self.client = None

    def tearDown(self):
        if self.client is not None:
            self.client.close()
        self._stop_server()
        self.tempdir.cleanup()

    def _start_server(self, *, definition: bool):
        socket_path = Path(self.tempdir.name) / "searchkerneld.sock"
        environment = os.environ.copy()
        environment.update({
            "LD_LIBRARY_PATH": os.pathsep.join(
                path for path in (str(SDK_LIB), str(VESPA_DEPS_LIB), environment.get("LD_LIBRARY_PATH"))
                if path
            ),
        })
        command = [
            str(DAEMON),
            "--data-root",
            str(self.index_path),
            "--listen",
            f"unix://{socket_path}",
        ]
        self.process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                output = self.process.stderr.read().decode(errors="replace")
                self.fail(f"searchkerneld exited during startup: {output}")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(str(socket_path))
                    endpoint = f"unix://{socket_path}"
                    if definition:
                        index = Index.create(
                            endpoint,
                            fields=self.definition,
                            target=self.target,
                        )
                        index.close()
                    return endpoint
                except OSError:
                    time.sleep(0.05)
        self.fail("searchkerneld did not open its gRPC socket")

    def _start_legacy_server(self):
        socket_path = Path(self.tempdir.name) / "legacy-searchkerneld.sock"
        definition_path = Path(self.tempdir.name) / "legacy-definition.fb"
        definition_path.write_bytes(pack_object(self.definition))
        environment = os.environ.copy()
        environment.update({
            "LD_LIBRARY_PATH": os.pathsep.join(
                path for path in (str(SDK_LIB), str(VESPA_DEPS_LIB), environment.get("LD_LIBRARY_PATH"))
                if path
            ),
        })
        command = [
            str(DAEMON),
            "--index",
            str(self.index_path),
            "--definition",
            str(definition_path),
            "--listen",
            f"unix://{socket_path}",
        ]
        self.process = subprocess.Popen(
            command,
            cwd=ROOT,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                output = self.process.stderr.read().decode(errors="replace")
                self.fail(f"legacy searchkerneld exited during startup: {output}")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(str(socket_path))
                    return f"unix://{socket_path}"
                except OSError:
                    time.sleep(0.05)
        self.fail("legacy searchkerneld did not open its gRPC socket")

    def _stop_server(self):
        if self.process is None:
            return
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        if self.process.stdout is not None:
            self.process.stdout.close()
        if self.process.stderr is not None:
            self.process.stderr.close()
        self.process = None

    # SKC-unverified: update.document_crud/basic kinds=e2e runtime=grpc
    # SKC-unverified: filter.textual/basic kinds=e2e runtime=grpc
    def test_generated_objects_reach_persisted_native_backend(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client)
        index.upsert("doc:1", {"title": "hello world", "category": "a", "price": 5})
        index.upsert("doc:2", {"title": "hello again", "category": "b", "price": 2})

        stored = index.get("doc:1").document
        stored_fields = {entry.key: entry.value.kind.value for entry in stored.fields}
        self.assertEqual(stored_fields["title"], "hello world")

        request = SearchRequestT(
            query=QuerySourceT(
                sourceType=QuerySourceKind.SearchKernelTextQuery,
                source=SearchKernelTextQueryT(text="hello"),
            ),
            limit=OptionalUInt32T(value=10),
        )
        result = index.search(request)
        self.assertEqual(result.totalHits.value, 2)
        self.assertEqual({child.node.id for child in [result.root.node.children[0], result.root.node.children[1]]}, {"doc:1", "doc:2"})

        filtered = index.search(SearchRequestT(
            query=QuerySourceT(
                sourceType=QuerySourceKind.SearchKernelTextQuery,
                source=SearchKernelTextQueryT(text="hello"),
            ),
            filters=FilterRequestT(
                textual=[FilterClauseT(
                    source="price <= :max",
                    bindings=[ValueEntryT(key="max", value=value(2))],
                )],
            ),
            limit=OptionalUInt32T(value=10),
        ))
        self.assertEqual(
            {child.node.id for child in filtered.root.node.children},
            {"doc:2"},
        )

        described = index.describe().index
        self.assertEqual(described.name, self.definition.name)
        self.assertEqual({field.name for field in described.fields}, {"title", "category", "price"})

        index.delete("doc:2")
        self.assertIsNone(index.get("doc:2").document)
        self.client.close()
        self.client = None
        self._stop_server()

        endpoint = self._start_server(definition=False)
        reopened = Index.open(endpoint, target=self.target)
        self.assertEqual(reopened.get("doc:1")["_id"], "doc:1")
        self.assertEqual({hit.id for hit in reopened.search("hello").execute().hits}, {"doc:1"})
        reopened.close()

    def test_search_builder_inspect_returns_canonical_resolved_request(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client, ergonomic=True)

        resolved = (
            index.search("hello")
            .search_fields("title")
            .sort("price", "asc", missing="last")
            .timeout(250)
            .language("en")
            .limit(3)
            .inspect()
        )

        self.assertEqual(resolved.limit, 3)
        self.assertEqual(resolved.offset, 0)
        self.assertEqual(resolved.timeoutMs.value, 250)
        self.assertEqual([field.field for field in resolved.searchFields], ["title"])
        self.assertEqual(resolved.queryModel.language, "en")
        self.assertEqual(len(resolved.sort), 1)

    def test_focused_updates_survive_restart_and_change_search_visibility(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client, ergonomic=True)
        index.upsert("doc:1", {"title": "quick fox", "price": 10})

        index.update("doc:1").set("title", "slow hare").execute()
        index.update("doc:1").increment("price", 5).execute()

        self.assertEqual(index.get("doc:1")["price"], 15)
        self.assertEqual(
            {hit.id for hit in index.search("hare").execute().hits},
            {"doc:1"},
        )
        self.assertEqual(index.search("quick").execute().hits, [])

        self.client.close()
        self.client = None
        self._stop_server()

        endpoint = self._start_server(definition=False)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        self.client.open_index()
        reopened = Index(self.client, ergonomic=True)
        self.assertEqual(reopened.get("doc:1")["price"], 15)
        self.assertEqual(
            {hit.id for hit in reopened.search("hare").execute().hits},
            {"doc:1"},
        )
        self.assertEqual(reopened.search("quick").execute().hits, [])

    def test_python_grpc_handles_frames_larger_than_default_limit(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        large_text = "alpha " * (5 * 1024 * 1024 // 6)
        analyze = AnalyzeRequestT(
            targetType=AnalyzeTarget.AnalyzeFieldTarget,
            target=AnalyzeFieldTargetT(field="title"),
            text=large_text,
        )
        request_frame = pack_request("analyze", analyze, target=self.target)
        self.assertGreater(len(request_frame), 4 * 1024 * 1024)
        response_frame = self.client.transport.call(request_frame)
        self.assertGreater(len(response_frame), 4 * 1024 * 1024)

    # SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=managed-local
    def test_managed_local_mode_uses_production_daemon_over_uds(self):
        index = Index.create(self.index_path, fields=self.definition)
        try:
            index.upsert("doc:1", {"title": "hello", "category": "a", "price": 5})
            self.assertEqual(index.get("doc:1")["_id"], "doc:1")
        finally:
            index.close()

    def test_managed_local_refuses_preexisting_unowned_data_root(self):
        root = Path(self.tempdir.name)
        managed_root = root / ".searchkernel-managed"
        managed_root.mkdir()
        sentinel = managed_root / "user-data.txt"
        sentinel.write_text("keep", encoding="utf-8")

        with self.assertRaisesRegex(ClientError, "not owned by Search Kernel"):
            Index.create(root / "index", fields=self.definition)

        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep")
        self.assertFalse((root / "index").exists())

    def test_managed_local_sibling_indexes_share_one_daemon_and_survive_handle_close(self):
        root = Path(self.tempdir.name)
        alpha = Index.create(root / "managed-a", fields=self.definition)
        beta_definition = IndexDefinitionT(
            name="managed_b",
            fields=[_field("payload_text", 1, matching=MatchingMode.Text)],
        )
        beta = Index.create(root / "managed-b", fields=beta_definition)
        try:
            self.assertIs(alpha._owner, beta._owner)
            alpha.upsert("doc-a", {"title": "alpha item", "category": "a", "price": 1})
            beta.upsert("doc-b", {"payload_text": "beta item"})
            alpha.close()
            self.assertIsNotNone(beta.get("doc-b"))
            beta.upsert("doc-c", {"payload_text": "still alive"})
            self.assertIsNotNone(beta.get("doc-c"))
        finally:
            alpha.close()
            beta.close()

    def test_managed_local_migrates_arbitrary_legacy_path_into_safe_data_root(self):
        root = Path(self.tempdir.name)
        self.index_path = root / "Search Index.v1"
        unrelated = root / "projects"
        unrelated.mkdir()
        (unrelated / "keep.txt").write_text("untouched", encoding="utf-8")

        endpoint = self._start_legacy_server()
        legacy = Index(Client(GrpcTransport(endpoint)), ergonomic=True)
        legacy.upsert("legacy-doc", {"title": "legacy layout", "category": "a", "price": 7})
        legacy.close()
        self._stop_server()

        reopened = Index.open(self.index_path)
        try:
            self.assertEqual(reopened.get("legacy-doc")["title"], "legacy layout")
            self.assertEqual(reopened._owner.path, root / ".searchkernel-managed")
            self.assertTrue((self.index_path / ".searchkernel-managed-target").is_file())
            self.assertEqual((unrelated / "keep.txt").read_text(encoding="utf-8"), "untouched")
            self.assertFalse((root / ".searchkernel-managed" / "projects").exists())
        finally:
            reopened.close()

    def test_shared_daemon_lifecycle_isolates_indexes_and_handle_close(self):
        endpoint = self._start_server(definition=False)
        alpha = Index.create(endpoint, target="products-a", fields=self.definition)
        beta_definition = IndexDefinitionT(
            name="products_b",
            fields=[_field("payload_text", 1, matching=MatchingMode.Text)],
        )
        beta = Index.create(endpoint, target="products-b", fields=beta_definition)
        try:
            alpha.upsert("doc-a", {"title": "alpha item", "category": "a", "price": 1})
            beta.upsert("doc-b", {"payload_text": "beta item"})
            self.assertIsNotNone(alpha.get("doc-a"))
            self.assertIsNone(alpha.get("doc-b"))
            self.assertIsNotNone(beta.get("doc-b"))
            self.assertIsNone(beta.get("doc-a"))

            with self.assertRaises(ClientError) as collision:
                Index.create(endpoint, target="products-a", fields=self.definition)
            self.assertEqual(collision.exception.code, "already_exists")
            with self.assertRaises(ClientError) as missing:
                Index.open(endpoint, target="missing-index")
            self.assertEqual(missing.exception.code, "not_found")

            first = Index.open(endpoint, target="products-a")
            second = Index.open(endpoint, target="products-a")
            first.close()
            self.assertIsNotNone(second.get("doc-a"))
            self.assertEqual({hit.id for hit in second.search("alpha").execute().hits}, {"doc-a"})
            second.upsert("doc-c", {"title": "another alpha", "category": "a", "price": 2})
            self.assertIsNotNone(second.get("doc-c"))
            second.close()

            closed_only = Index.open(endpoint, target="products-b")
            closed_only.close()
            reopened_beta = Index.open(endpoint, target="products-b")
            self.assertIsNotNone(reopened_beta.get("doc-b"))
            reopened_beta.close()

            Index.delete_index(endpoint, target="products-a")
            with self.assertRaises(ClientError) as deleted:
                Index.open(endpoint, target="products-a")
            self.assertEqual(deleted.exception.code, "not_found")
            still_usable_beta = Index.open(endpoint, target="products-b")
            self.assertIsNotNone(still_usable_beta.get("doc-b"))
            still_usable_beta.close()

            with self.assertRaises(ClientError) as missing_delete:
                Index.delete_index(endpoint, target="products-a")
            self.assertEqual(missing_delete.exception.code, "not_found")
        finally:
            alpha.close()
            beta.close()

    def test_managed_local_delete_uses_path_identity_and_preserves_sibling(self):
        root = Path(self.tempdir.name)
        alpha_path = root / "managed-alpha"
        beta_path = root / "managed-beta"

        alpha = Index.create(alpha_path, fields=self.definition)
        beta = Index.create(beta_path, fields=self.definition)
        try:
            alpha.upsert("alpha-doc", {"title": "alpha", "category": "a", "price": 1})
            beta.upsert("beta-doc", {"title": "beta", "category": "b", "price": 2})
        finally:
            alpha.close()
            beta.close()

        Index.delete_index(alpha_path)
        self.assertFalse(alpha_path.exists(), "successful managed delete should remove only its marker path")

        with self.assertRaises(ClientError) as deleted:
            Index.open(alpha_path)
        self.assertEqual(deleted.exception.code, "not_found")

        surviving = Index.open(beta_path)
        try:
            self.assertIsNotNone(surviving.get("beta-doc"))
        finally:
            surviving.close()

        with self.assertRaises(ClientError) as missing_delete:
            Index.delete_index(alpha_path)
        self.assertEqual(missing_delete.exception.code, "not_found")

        Index.delete_index(beta_path)
        self.assertFalse(beta_path.exists())

    def test_ranking_profile_reaches_vespa_and_survives_reopen(self):
        self.definition = IndexDefinitionT(
            name="ranking_e2e",
            fields=[
                _field("title", 1, matching=MatchingMode.Text),
                _field("popularity", 5),
            ],
            rankProfiles=[
                RankProfileDefinitionT(
                    name="product_search",
                    firstPhase=RankPhaseDefinitionT(
                        expression="bm25(title) + query(boost) * attribute(popularity)",
                    ),
                    matchFeatures=[
                        "bm25(title)",
                        "query(boost)",
                        "attribute(popularity)",
                    ],
                ),
            ],
        )
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client)
        index.upsert("d1", {"title": "fox", "popularity": 10})
        index.upsert("d2", {"title": "fox fox", "popularity": 5})

        def search_with_input(boost):
            return index.search(
                SearchRequestT(
                    query=QuerySourceT(
                        sourceType=QuerySourceKind.SearchKernelTextQuery,
                        source=SearchKernelTextQueryT(text="fox"),
                    ),
                    ranking=RankingRequestT(
                        profile="product_search",
                    ),
                    inputs=[
                        QueryInputT(
                            name="boost",
                            inputType=QueryInputKind.LiteralQueryInput,
                            input=LiteralQueryInputT(value=value(boost)),
                        )
                    ],
                    limit=OptionalUInt32T(value=10),
                )
            )

        def search_with_property(boost):
            return index.search(
                SearchRequestT(
                    query=QuerySourceT(
                        sourceType=QuerySourceKind.SearchKernelTextQuery,
                        source=SearchKernelTextQueryT(text="fox"),
                    ),
                    ranking=RankingRequestT(
                        profile="product_search",
                        properties=[StringEntryT(key="boost", value=str(boost))],
                    ),
                    limit=OptionalUInt32T(value=10),
                )
            )

        positive = search_with_input(2.0)
        negative = search_with_input(-1.0)
        raw_property = search_with_property(2.0)
        self.assertEqual(positive.root.node.children[0].node.id, "d1")
        self.assertEqual(negative.root.node.children[0].node.id, "d2")
        self.assertEqual(raw_property.root.node.children[0].node.id, "d1")
        self.assertEqual(
            [child.node.id for child in positive.root.node.children],
            [child.node.id for child in raw_property.root.node.children],
        )
        self.assertEqual(
            [child.node.relevance.value.value for child in positive.root.node.children],
            [child.node.relevance.value.value for child in raw_property.root.node.children],
        )
        self.assertEqual(
            {entry.key for entry in positive.root.node.children[0].node.matchFeatures},
            {"bm25(title)", "query(boost)", "attribute(popularity)"},
        )

        self.client.close()
        self.client = None
        self._stop_server()
        endpoint = self._start_server(definition=False)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        self.client.open_index()
        reopened = Index(self.client)
        surviving = reopened.search(
            SearchRequestT(
                query=QuerySourceT(
                    sourceType=QuerySourceKind.SearchKernelTextQuery,
                    source=SearchKernelTextQueryT(text="fox"),
                ),
                ranking=RankingRequestT(
                    profile="product_search",
                    properties=[StringEntryT(key="boost", value="2.0")],
                ),
                limit=OptionalUInt32T(value=10),
            )
        )
        self.assertEqual(surviving.root.node.children[0].node.id, "d1")

    # SKC: query.phrase/basic kinds=ergonomic,e2e runtime=grpc
    # SKC: query.proximity/basic kinds=ergonomic,e2e runtime=grpc
    # SKC: query.fuzzy/basic kinds=ergonomic,e2e runtime=grpc
    # SKC: query.weighted_and_wand/basic kinds=ergonomic,e2e runtime=grpc
    def test_typed_query_families_reach_vespa(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client)
        index.upsert("d1", {"title": "quick brown fox", "category": "fox", "price": 5})
        index.upsert("d2", {"title": "brown quick fox", "category": "fox", "price": 2})
        index.upsert("d3", {"title": "quick fox", "category": "fox", "price": 8})
        index.upsert("d4", {"title": "fox", "category": "fox", "price": 1})

        def ids(query):
            result = index.search(SearchRequestT(query=query, limit=OptionalUInt32T(value=10)))
            return [child.node.id for child in result.root.node.children]

        self.assertEqual(ids(q.contains("title", q.phrase("quick", "brown"))), ["d1"])
        self.assertEqual(set(ids(q.contains("title", q.near("quick", "brown")))), {"d1", "d2"})
        self.assertEqual(ids(q.contains("title", q.onear("quick", "brown"))), ["d1"])
        self.assertEqual(set(ids(q.contains("category", q.fuzzy("foxx")))), {"d1", "d2", "d3", "d4"})
        self.assertEqual(
            set(ids(q.rank(q.contains("title", "fox"), q.contains("title", "quick")))),
            {"d1", "d2", "d3", "d4"},
        )
        self.assertEqual(
            set(ids(q.wand("title", {"quick": 10, "fox": 1}))),
            {"d1", "d2", "d3", "d4"},
        )

    # SKC: query.same_element/basic kinds=ergonomic,e2e runtime=grpc
    def test_same_element_query_reaches_vespa(self):
        self.definition = IndexDefinitionT(
            name="same_element_e2e",
            fields=[
                FieldDefinitionT(
                    name="people",
                    dataType=DataTypeT(
                        kindType=DataTypeKind.ArrayDataType,
                        kind=ArrayDataTypeT(
                            elementType=DataTypeT(
                                kindType=DataTypeKind.StructDataType,
                                kind=StructDataTypeT(
                                    fields=[
                                        StructFieldDefinitionT(
                                            name="first",
                                            dataType=DataTypeT(
                                                kindType=DataTypeKind.PrimitiveDataType,
                                                kind=PrimitiveDataTypeT(kind=1),
                                            ),
                                        ),
                                        StructFieldDefinitionT(
                                            name="last",
                                            dataType=DataTypeT(
                                                kindType=DataTypeKind.PrimitiveDataType,
                                                kind=PrimitiveDataTypeT(kind=1),
                                            ),
                                        ),
                                    ]
                                ),
                            )
                        ),
                    ),
                    stored=OptionalBoolT(False),
                    searchable=OptionalBoolT(False),
                )
            ],
        )
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client)
        index.upsert(
            "d1",
            {
                "people": [
                    {"first": "Ada", "last": "Lovelace"},
                    {"first": "Grace", "last": "Hopper"},
                ]
            },
        )
        index.upsert("d2", {"people": [{"first": "Ada", "last": "Hopper"}]})
        result = index.search(
            SearchRequestT(
                query=q.contains(
                    "people",
                    q.same_element(
                        q.contains("first", "Ada"),
                        q.contains("last", "Lovelace"),
                    ),
                ),
                limit=OptionalUInt32T(value=10),
            )
        )
        self.assertEqual([child.node.id for child in result.root.node.children], ["d1"])

    # SKC: query.geo/basic kinds=ergonomic,e2e runtime=grpc
    def test_geo_queries_reach_vespa(self):
        self.definition = IndexDefinitionT(
            name="geo_e2e",
            fields=[
                FieldDefinitionT(
                    name="location",
                    dataType=DataTypeT(
                        kindType=DataTypeKind.PositionDataType,
                        kind=PositionDataTypeT(),
                    ),
                    stored=OptionalBoolT(False),
                )
            ],
        )
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client)
        index.upsert("inside", {"location": {"latitude": 37.3, "longitude": -121.9}})
        index.upsert("outside", {"location": {"latitude": 40.7, "longitude": -74.0}})

        location = index.search(
            SearchRequestT(
                query=q.geo_location(
                    "location", lat=37.3, lon=-121.9, radius="10km"
                ),
                limit=OptionalUInt32T(value=10),
            )
        )
        box = index.search(
            SearchRequestT(
                query=q.geo_box(
                    "location", south=37.0, west=-122.2, north=37.6, east=-121.5
                ),
                limit=OptionalUInt32T(value=10),
            )
        )
        self.assertEqual([child.node.id for child in location.root.node.children], ["inside"])
        self.assertEqual([child.node.id for child in box.root.node.children], ["inside"])

    # SKC: result.docsum_field_projection/basic kinds=ergonomic,e2e runtime=grpc
    # SKC: result.highlighting/basic kinds=ergonomic,e2e runtime=grpc
    def test_ergonomic_summary_projection_and_highlighting_reach_native_docsums(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint), target=self.target)
        index = Index(self.client, ergonomic=True)
        index.upsert("d1", {"title": "highlight me", "category": "a", "price": 5})
        result = (
            index.search("highlight")
            .summary("default")
            .fields("category")
            .highlight("title")
            .execute()
        )

        self.assertEqual(result.total_hits, 1)
        self.assertEqual([hit.id for hit in result.hits], ["d1"])
        self.assertEqual(result.hits[0].fields, {"category": "a"})
        self.assertEqual(result.hits[0].highlights[0]["field"], "title")
        self.assertEqual(result.hits[0].highlights[0]["fragments"][0]["text"], "highlight me")
        self.assertEqual(
            result.hits[0].highlights[0]["fragments"][0]["matches"],
            [(0, 9)],
        )


def load_tests(loader, tests, pattern):
    if os.environ.get("SEARCHKERNEL_RUN_NATIVE_E2E") == "1":
        return tests

    class NativeE2EOptInRequired(unittest.TestCase):
        @unittest.skip("requires explicit native Vespa E2E opt-in")
        def test_native_e2e_requires_opt_in(self):
            pass

    return loader.loadTestsFromTestCase(NativeE2EOptInRequired)


if __name__ == "__main__":
    unittest.main()
