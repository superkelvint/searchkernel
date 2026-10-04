import { mkdtempSync, mkdirSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const directory = mkdtempSync(join(tmpdir(),'instantsearch-package-'));
const npm = process.env.NPM ?? 'npm';
try {
  const [archive] = JSON.parse(execFileSync(npm,['pack','--json','--pack-destination',directory],{encoding:'utf8'}));
  const application = join(directory,'external-application');
  mkdirSync(application);
  execFileSync(npm,['install','--prefix',application,'--offline','--ignore-scripts','--no-audit','--no-fund',join(directory,archive.filename)],{stdio:'pipe'});
  const manifest = JSON.parse(readFileSync(join(application,'node_modules/@searchkernel/instantsearch/package.json'),'utf8'));
  assert.equal(Object.keys(manifest.dependencies ?? {}).length,0);
  execFileSync(process.execPath,['--input-type=module','-e',`import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';
    const client = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search'});
    const result = await client.search([]);
    if (result.results.length !== 0) throw Error('package contract drift');`],{cwd:application,stdio:'pipe'});
  console.log('Packed adapter imports in an external application without searchkernel-web or runtime dependencies: PASS');
} finally { rmSync(directory,{recursive:true,force:true}); }
