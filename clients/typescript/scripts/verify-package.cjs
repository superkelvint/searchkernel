const { execFileSync } = require("node:child_process");

function fail(message) {
  console.error(message);
  process.exit(1);
}

const packed = JSON.parse(execFileSync(
  "npm",
  ["pack", "--dry-run", "--json", "--ignore-scripts"],
  { encoding: "utf8" },
));
const files = new Set((packed[0]?.files ?? []).map((file) => file.path));
for (const required of ["dist/index.js", "dist/index.d.ts", "dist/low-level.js", "dist/low-level.d.ts"]) {
  if (!files.has(required)) fail(`packed package is missing ${required}`);
}

require("@searchkernel/client");
require("@searchkernel/client/low-level");

(async () => {
  await import("@searchkernel/client");
  await import("@searchkernel/client/low-level");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
