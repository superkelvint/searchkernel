// @vitest-environment jsdom
import { it, expect } from 'vitest';
import { frameworkCases } from './framework-registry.mjs';
import { qualifyFramework } from './framework-acceptance';
import contract from '../src/compatibility.json';

for(const [name, qualification] of Object.entries(frameworkCases)) {
  it(`qualifies ${name} (${qualification.id}) through production adapter/endpoint/IR/Vespa`,async()=>{
    expect((contract.frameworks as any)[name].acceptance).toBe(qualification.id);
    const {mount}=await qualification.load();
    await qualifyFramework(name as keyof typeof frameworkCases,mount);
  },60000);
}
