import { mkdtempSync, copyFileSync, mkdirSync, readFileSync, writeFileSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { expect, it } from 'vitest';

it.each(['pin','coverage','inventory','documentation','source'])('rejects %s compatibility drift', kind => {
  const root = mkdtempSync(join(tmpdir(),'instantsearch-guard-'));
  try {
    mkdirSync(join(root,'scripts')); mkdirSync(join(root,'src'));
    for (const file of ['package.json','package-lock.json','README.md','src/compatibility.json','scripts/check-contract.mjs']) {
      copyFileSync(new URL(`../${file}`,import.meta.url),join(root,file));
    }
    symlinkSync(new URL('../node_modules',import.meta.url).pathname,join(root,'node_modules'),'dir');
    const contractFile = join(root,'src/compatibility.json');
    const contract = JSON.parse(readFileSync(contractFile,'utf8'));
    if (kind === 'pin') contract.packages['instantsearch.js'] = '0.0.0';
    if (kind === 'coverage') delete contract.widgets.hits.acceptance;
    if (kind === 'inventory') delete contract.widgets.hits;
    if (kind === 'source') contract.upstreamSources['instantsearch.js/es/lib/utils/escape-highlight.js'] = 'wrong';
    if (kind === 'documentation') writeFileSync(join(root,'README.md'),readFileSync(join(root,'README.md'),'utf8').replace('| hits |','| wrong |'));
    writeFileSync(contractFile,JSON.stringify(contract));
    const result = spawnSync(process.execPath,[join(root,'scripts/check-contract.mjs')],{encoding:'utf8'});
    expect(result.status).not.toBe(0);
  } finally {rmSync(root,{recursive:true,force:true});}
});
