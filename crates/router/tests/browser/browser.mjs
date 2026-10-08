import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const directory = dirname(fileURLToPath(import.meta.url));
const pkg = process.env.CREAMUI_ROUTER_PKG ?? join(directory, 'pkg');
const html = `<!doctype html><script type="module">
import init, * as router from '/router.js';
await init(); window.router = router; router.start(); window.ready = true;
</script>`;
const server = createServer(async (request, response) => {
  try {
    const path = new URL(request.url, 'http://localhost').pathname;
    if (path === '/router.js' || path === '/router_bg.wasm') {
      response.setHeader('Content-Type', path.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
      response.end(await readFile(join(pkg, path.slice(1))));
    } else { response.setHeader('Content-Type', 'text/html'); response.end(html); }
  } catch (error) { response.writeHead(500).end(String(error)); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
try {
  browser = await chromium.launch({ executablePath: process.env.CHROMIUM_BIN, args: ['--no-sandbox'] });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const origin = `http://127.0.0.1:${server.address().port}`;
  await page.goto(`${origin}/users/42?tab=info#bio`);
  await page.waitForFunction(() => window.ready);
  assert.equal(await page.evaluate(() => router.current_url()), '/users/42?tab=info#bio');
  assert.equal(await page.evaluate(() => router.param_id()), '42');
  await page.evaluate(() => { window.marker = 123; router.navigate('/users/7?tab=settings'); });
  assert.equal(await page.evaluate(() => location.pathname + location.search), '/users/7?tab=settings');
  assert.equal(await page.evaluate(() => router.observed_url()), '/users/7?tab=settings');
  await page.evaluate(() => router.replace('/users/a%2Fb#details'));
  assert.equal(await page.evaluate(() => router.param_id()), 'a/b');
  await page.evaluate(() => router.back());
  await page.waitForFunction(() => router.current_url() === '/users/42?tab=info#bio');
  assert.equal(await page.evaluate(() => router.can_forward()), true);
  await page.evaluate(() => router.forward());
  await page.waitForFunction(() => router.observed_url() === '/users/a%2Fb#details');
  assert.equal(await page.evaluate(() => window.marker), 123); // No document reload.
  await page.evaluate(() => history.back()); // Native browser traversal also reacts.
  await page.waitForFunction(() => router.observed_url() === '/users/42?tab=info#bio');
  await page.evaluate(() => router.navigate('/users/99'));
  assert.equal(await page.evaluate(() => router.can_forward()), false);
  assert.equal(await page.evaluate(() => {
    try { router.navigate('https://example.com/'); return false; } catch { return true; }
  }), true);
  assert.equal(await page.evaluate(() => router.current_url()), '/users/99');
  await page.reload();
  await page.waitForFunction(() => window.ready);
  assert.equal(await page.evaluate(() => router.current_url()), '/users/99');
  await page.evaluate(() => { router.dispose(); history.pushState(null, '', '/disposed'); dispatchEvent(new PopStateEvent('popstate')); });
  assert.deepEqual(errors, []);
  console.log('Browser router checks passed: initial URL, params, reactive navigation, replace, Back/Forward, reload, rejection, disposal.');
} finally {
  await browser?.close();
  await new Promise(resolve => server.close(resolve));
}
