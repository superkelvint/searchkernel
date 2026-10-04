import { existsSync } from 'node:fs';
import { chromium } from 'playwright';
if (!existsSync(chromium.executablePath())) {
  throw new Error('Pinned Chromium is absent. Provision the declared CI toolchain image; see BUILDING.md InstantSearch compatibility. Acceptance never installs system dependencies.');
}
const browser = await chromium.launch({headless:true,args:['--no-sandbox']});
await browser.close();
console.log('Pinned Chromium prerequisite: PASS');
