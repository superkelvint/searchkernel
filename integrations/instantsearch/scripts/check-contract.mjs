import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const require = createRequire(import.meta.url);
const root = new URL('../', import.meta.url);
const read = path => JSON.parse(readFileSync(new URL(path, root), 'utf8'));
const contract = read('src/compatibility.json');
const manifest = read('package.json');
const lock = read('package-lock.json');
for (const [source,digest] of Object.entries(contract.upstreamSources ?? {})) {
  const bytes = readFileSync(new URL(`node_modules/${source}`,root));
  assert.equal(createHash('sha256').update(bytes).digest('hex'),digest,`pinned upstream source drift: ${source}`);
}
for (const [name, version] of Object.entries(contract.packages)) {
  assert.equal(manifest.devDependencies[name], version, `contract pin drift: ${name}`);
  assert.equal(lock.packages[`node_modules/${name}`].version, version, `lock pin drift: ${name}`);
  assert.equal(read(`node_modules/${name}/package.json`).version, version, `installed pin drift: ${name}`);
}
const upstream = readFileSync(require.resolve('instantsearch.js/es/widgets/index.js'), 'utf8');
const names = [...upstream.matchAll(/export \{ default as (\w+) \}/g)].map(match => match[1]);
for (const group of upstream.matchAll(/export \{ ([\w, ]+) \};/g)) {
  names.push(...group[1].split(',').map(name=>name.trim()));
}
assert.deepEqual(Object.keys(contract.widgets).sort(), [...new Set(names)].sort(), 'upstream widget inventory drift');
for (const [name, widget] of Object.entries({...contract.widgets,...contract.helpers})) {
  assert.ok(['supported', 'constrained', 'unsupported'].includes(widget.status), `invalid status: ${name}`);
  if (widget.status !== 'unsupported') assert.ok(widget.acceptance?.length, `missing acceptance: ${name}`);
}
assert.equal(manifest.dependencies,undefined,'production adapter must not depend on dashboard/framework runtimes');
const labels = {supported:'supported',constrained:'supported with documented constraints',unsupported:'unsupported'};
const matrix = ['<!-- compatibility-matrix:start -->',
  '| Pinned upstream package | Version |','| --- | --- |',
  ...Object.entries(contract.packages).map(([name,version])=>`| ${name} | ${version} |`), '',
  '| Upstream widget | Status | Coverage / reason |','| --- | --- | --- |',
  ...Object.entries(contract.widgets).map(([name,widget])=>`| ${name} | ${labels[widget.status]} | ${widget.acceptance?.join(', ') ?? widget.reason} |`),
  '<!-- compatibility-matrix:end -->'].join('\n');
const documentation = new URL('README.md',root);
const original = readFileSync(documentation,'utf8');
const generated = original.replace(/<!-- compatibility-matrix:start -->[\s\S]*?<!-- compatibility-matrix:end -->/,matrix);
if (process.argv.includes('--write')) writeFileSync(documentation,generated);
else assert.equal(original,generated,'generated compatibility matrix drift: run node scripts/check-contract.mjs --write');
console.log(`Pinned InstantSearch contract and ${names.length} upstream widget exports: PASS`);
