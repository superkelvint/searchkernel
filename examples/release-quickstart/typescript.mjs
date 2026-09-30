import { Index } from "@searchkernel/client/node";

const endpoint = process.env.SEARCHKERNEL_ENDPOINT ?? "http://127.0.0.1:8080";
const target = process.env.SEARCHKERNEL_TARGET;
if (!target) throw new Error("Set SEARCHKERNEL_TARGET to a logical index name");
const index = Index.connect(endpoint, target);

try {
  await index.insert({
    _id: "quickstart-typescript",
    title: "quickstart example",
    kind: "quickstart",
    score: 1n,
  });
  const hits = (await index.search("quickstart").execute()).hits;
  if (hits.map((hit) => hit.id).join(",") !== "quickstart-typescript") {
    throw new Error("quickstart search did not return the inserted document");
  }
} finally {
  await index.close();
}

console.log("TypeScript release quickstart PASS");
