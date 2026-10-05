import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { frameworkCases, frameworkSuite } from '../tests/framework-registry.mjs';
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
const supported = Object.entries({...contract.widgets,...contract.helpers}).filter(([, value])=>value.status !== 'unsupported').map(([name])=>name).sort();
assert.ok(manifest.scripts['test:engine'].split(/\s+/).includes(frameworkSuite),'framework suite missing from canonical engine acceptance');
readFileSync(new URL(frameworkSuite,root));
for(const name of ['playwright','@playwright/test','puppeteer']) {
  assert.equal(manifest.devDependencies[name],undefined,'JSDOM qualification must not require a browser runtime');
}
assert.deepEqual(Object.keys(contract.frameworks).sort(),Object.keys(frameworkCases).sort(),'framework qualification registry drift');
for (const [name, framework] of Object.entries(contract.frameworks)) {
  assert.ok(['not-yet-qualified','constrained','unsupported'].includes(framework.status),`framework status: ${name}`);
  if(framework.status !== 'constrained') continue;
  const qualification=frameworkCases[name];
  assert.equal(framework.acceptance,qualification.id,`framework acceptance missing: ${name}`);
  assert.equal(typeof qualification.load,'function',`framework loader missing: ${name}`);
  assert.equal(framework.root,qualification.root,`framework root drift: ${name}`);
  assert.deepEqual(framework.components,qualification.components,`framework component mapping drift: ${name}`);
  assert.deepEqual(Object.keys(framework.components).sort(),supported,`framework profile coverage drift: ${name}`);
}
for(const name of ['react-instantsearch','react-instantsearch-core','vue-instantsearch']) {
  const dependency=read(`node_modules/${name}/package.json`).dependencies;
  assert.equal(dependency['instantsearch.js'],contract.packages['instantsearch.js'],`framework InstantSearch ecosystem drift: ${name}`);
  if(name === 'react-instantsearch-core') assert.equal(dependency['algoliasearch-helper'],contract.packages['algoliasearch-helper'],'framework helper ecosystem drift');
}
for(const [path, value] of Object.entries(lock.packages)) {
  for(const name of ['instantsearch.js','algoliasearch-helper','react-instantsearch-core','react','react-dom','vue']) {
    if(path.endsWith(`node_modules/${name}`)) assert.equal(value.version,contract.packages[name],`nested framework ecosystem drift: ${path}`);
  }
}
assert.equal(manifest.dependencies,undefined,'production adapter must not depend on dashboard/framework runtimes');
const labels = {supported:'supported',constrained:'supported with documented constraints',unsupported:'unsupported','not-yet-qualified':'not yet qualified'};
const matrix = ['<!-- compatibility-matrix:start -->',
  '| Pinned upstream package | Version |','| --- | --- |',
  ...Object.entries(contract.packages).map(([name,version])=>`| ${name} | ${version} |`), '',
  '| Framework | Status | Qualification / constraints |','| --- | --- | --- |',
  ...Object.entries(contract.frameworks).map(([name,value])=>`| ${name} | ${labels[value.status]} | ${value.acceptance ?? 'none'}; ${value.constraints ?? value.reason} |`), '',
  '| Underlying capability | React surface | Vue 3 surface |','| --- | --- | --- |',
  ...supported.map(name=>`| ${name} | ${contract.frameworks['react-instantsearch'].components?.[name] ?? 'not qualified'} | ${contract.frameworks['vue-instantsearch'].components?.[name] ?? 'not qualified'} |`), '',
  '| Upstream widget | Status | Coverage / constraints / reason |','| --- | --- | --- |',
  ...Object.entries(contract.widgets).map(([name,widget])=>`| ${name} | ${labels[widget.status]} | ${widget.acceptance ? widget.acceptance.join(', ') + '; ' + widget.constraints : widget.reason} |`),
  '<!-- compatibility-matrix:end -->'].join('\n');
const documentation = new URL('README.md',root);
const original = readFileSync(documentation,'utf8');
const generated = original.replace(/<!-- compatibility-matrix:start -->[\s\S]*?<!-- compatibility-matrix:end -->/,matrix);
if (process.argv.includes('--write')) writeFileSync(documentation,generated);
else assert.equal(original,generated,'generated compatibility matrix drift: run node scripts/check-contract.mjs --write');
console.log(`Pinned InstantSearch contract and ${names.length} upstream widget exports: PASS`);
