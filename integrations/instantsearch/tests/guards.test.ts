import { mkdtempSync, copyFileSync, mkdirSync, readFileSync, writeFileSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { expect, it } from 'vitest';

it.each(['pin','coverage','inventory','documentation','source','framework-pin','framework-core-pin','runtime-pin','framework-acceptance','framework-component','framework-registry','framework-suite','framework-documentation','nested-ecosystem','underlying-profile','missing-suite','browser-runtime'])('rejects %s compatibility drift', kind => {
  const root = mkdtempSync(join(tmpdir(),'instantsearch-guard-'));
  try {
    mkdirSync(join(root,'scripts')); mkdirSync(join(root,'src')); mkdirSync(join(root,'tests'));
    for (const file of ['package.json','package-lock.json','README.md','src/compatibility.json','scripts/check-contract.mjs','tests/framework-registry.mjs','tests/framework-real-engine.test.ts']) {
      copyFileSync(new URL(`../${file}`,import.meta.url),join(root,file));
    }
    symlinkSync(new URL('../node_modules',import.meta.url).pathname,join(root,'node_modules'),'dir');
    const baseline=spawnSync(process.execPath,[join(root,'scripts/check-contract.mjs')],{encoding:'utf8'});
    expect(baseline.status,baseline.stderr).toBe(0);
    const contractFile = join(root,'src/compatibility.json');
    const contract = JSON.parse(readFileSync(contractFile,'utf8'));
    if(kind === 'missing-suite') rmSync(join(root,'tests/framework-real-engine.test.ts'));
    if(kind === 'browser-runtime') {
      const path=join(root,'package.json');const manifest=JSON.parse(readFileSync(path,'utf8'));
      manifest.devDependencies.playwright='1.56.1';writeFileSync(path,JSON.stringify(manifest));
    }
    if(kind === 'framework-pin') contract.packages['react-instantsearch']='0.0.0';
    if(kind === 'framework-core-pin') contract.packages['react-instantsearch-core']='0.0.0';
    if(kind === 'runtime-pin') contract.packages.vue='0.0.0';
    if(kind === 'framework-acceptance') delete contract.frameworks['react-instantsearch'].acceptance;
    if(kind === 'framework-component') delete contract.frameworks['vue-instantsearch'].components.hits;
    if(kind === 'framework-registry') {
      const registry=join(root,'tests/framework-registry.mjs');
      writeFileSync(registry,readFileSync(registry,'utf8').replace(/load: \(\) => import\([^\n]+\),/,'load: undefined,'));
    }
    if(kind === 'framework-suite') {
      const path=join(root,'package.json'); const manifest=JSON.parse(readFileSync(path,'utf8'));
      manifest.scripts['test:engine']='vitest run tests/real-engine.test.ts';writeFileSync(path,JSON.stringify(manifest));
    }
    if(kind === 'framework-documentation') {
      const path=join(root,'README.md');writeFileSync(path,readFileSync(path,'utf8').replace('| AisSearchBox |','| Wrong |'));
    }
    if(kind === 'nested-ecosystem') {
      const path=join(root,'package-lock.json');const lock=JSON.parse(readFileSync(path,'utf8'));
      lock.packages['node_modules/vue-instantsearch/node_modules/instantsearch.js']={version:'0.0.0'};writeFileSync(path,JSON.stringify(lock));
    }
    if(kind === 'underlying-profile') contract.widgets.rangeInput={status:'constrained',acceptance:['missing-real-range-test'],constraints:'not actually covered'};
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
