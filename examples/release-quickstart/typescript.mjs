import { Index, Integer, Keyword, Text } from "@searchkernel/client/node";

const endpoint = process.env.SEARCHKERNEL_ENDPOINT ?? "127.0.0.1:50051";
const target = process.env.SEARCHKERNEL_TARGET ?? "quickstart";
const fields = {
  title: new Text({ stored: true, searchable: true }),
  kind: new Keyword({ stored: true, filterable: true }),
  score: new Integer({ stored: true, sortable: true }),
};
const documents = [
  { _id: "quickstart-a", title: "quickstart alpha", kind: "guide", score: 2n },
  { _id: "quickstart-b", title: "quickstart beta", kind: "guide", score: 1n },
  { _id: "quickstart-c", title: "quickstart other", kind: "other", score: 0n },
];

async function matchingIds(index) {
  const results = await index
    .search("quickstart")
    .filter("kind = 'guide'")
    .sort("score", "asc")
    .execute();
  return results.hits.map((hit) => hit.id);
}

let index = await Index.create(endpoint, target, { fields });
try {
  for (const document of documents) await index.insert(document);
  const ids = await matchingIds(index);
  if (ids.join(",") !== "quickstart-b,quickstart-a") {
    throw new Error(`unexpected filtered/sorted ids: ${ids.join(",")}`);
  }
} finally {
  await index.close();
}

index = await Index.open(endpoint, target);
try {
  const document = await index.get("quickstart-b");
  if (document?.score !== 1n) throw new Error("reopened index lost quickstart-b");
  const ids = await matchingIds(index);
  if (ids.join(",") !== "quickstart-b,quickstart-a") {
    throw new Error(`unexpected reopened ids: ${ids.join(",")}`);
  }
} finally {
  await index.close();
}

console.log("TypeScript release quickstart PASS");
