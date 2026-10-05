import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const directory = dirname(fileURLToPath(import.meta.url));
const pkg = process.env.CREAMUI_BROWSER_PKG ?? join(directory, 'pkg');
const html = `<!doctype html><button id="file">Choose file</button>
<script type="module">
import init, * as bridge from '/bridges.js';
await init();
window.bridge = bridge;
bridge.rebuild_editor(false);
document.querySelector('#file').addEventListener('click', bridge.select_file);
document.addEventListener('keydown', event => {
  if (event.key.length === 1 || ['Home', 'End'].includes(event.key)) {
    event.preventDefault();
    bridge.keypress(event.key, event.ctrlKey);
  }
});
</script>`;
const server = createServer(async (request, response) => {
  try {
    const name = request.url?.slice(1);
    if (!name) {
      response.setHeader('Content-Type', 'text/html');
      response.end(html);
    } else if (name === 'bridges.js' || name === 'bridges_bg.wasm') {
      response.setHeader('Content-Type', name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
      response.end(await readFile(join(pkg, name)));
    } else {
      response.writeHead(404).end();
    }
  } catch (error) {
    response.writeHead(500).end(String(error));
  }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM_BIN,
  args: ['--no-sandbox'],
});
try {
  const context = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.waitForFunction(() => Boolean(window.bridge));

  for (const multiline of [false, true]) {
    await page.evaluate(multiline => {
      bridge.set_text('héllo');
      bridge.rebuild_editor(multiline);
    }, multiline);
    await page.keyboard.press('Control+A');
    await page.keyboard.press('Control+C');
    await page.waitForFunction(async () => await navigator.clipboard.readText() === 'héllo');
    const paste = multiline ? '🌱\nsecond line' : '🌱';
    await page.evaluate(text => navigator.clipboard.writeText(text), paste);
    await page.keyboard.press('Control+V');
    await page.waitForFunction(text => bridge.editor_text() === text, paste);
  }

  await page.evaluate(() => {
    bridge.set_text('base');
    bridge.rebuild_editor(false);
    Object.defineProperty(navigator.clipboard, 'readText', {
      configurable: true,
      value: () => new Promise(resolve => { window.resolvePaste = resolve; }),
    });
  });
  await page.keyboard.press('Control+V');
  await page.evaluate(() => bridge.rebuild_editor(false));
  await page.evaluate(() => resolvePaste(' delayed'));
  await page.waitForFunction(() => bridge.editor_text() === 'base delayed');
  await page.evaluate(() => bridge.rebuild_editor(false));

  await page.keyboard.press('Control+V');
  await page.keyboard.press('x');
  await page.evaluate(() => resolvePaste(' stale'));
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  assert.equal(await page.evaluate(() => bridge.editor_text()), 'base delayedx');

  await page.evaluate(() => {
    Object.defineProperty(navigator.clipboard, 'readText', {
      configurable: true,
      value: () => Promise.reject(new DOMException('Denied', 'NotAllowedError')),
    });
  });
  await page.keyboard.press('Control+V');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  assert.equal(await page.evaluate(() => bridge.editor_text()), 'base delayedx');

  await page.evaluate(() => {
    Object.defineProperty(navigator.clipboard, 'readText', {
      configurable: true,
      value: () => Promise.resolve(''),
    });
  });
  await page.keyboard.press('Control+A');
  await page.keyboard.press('Control+V');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  assert.equal(await page.evaluate(() => bridge.editor_text()), 'base delayedx');

  for (let index = 0; index < 3; index += 1) {
    const chooserPromise = page.waitForEvent('filechooser');
    await page.locator('#file').click();
    const chooser = await chooserPromise;
    const accept = await page.locator('input[type=file]').getAttribute('accept');
    assert.equal(accept, '.md,.txt');
    await chooser.setFiles({ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from(`hello 🌱 ${index}`) });
    await page.waitForFunction(expected => bridge.file_text() === expected, `hello 🌱 ${index}`);
    assert.equal(await page.evaluate(() => bridge.file_name()), 'notes.txt');
    assert.equal(await page.evaluate(() => bridge.file_error()), '');
    assert.equal(await page.locator('input[type=file]').count(), 0);
  }

  const cancellation = page.waitForEvent('filechooser');
  await page.locator('#file').click();
  await cancellation;
  await page.locator('input[type=file]').dispatchEvent('cancel');
  await page.waitForFunction(() => document.querySelectorAll('input[type=file]').length === 0);
  assert.equal(await page.evaluate(() => bridge.file_text()), 'hello 🌱 2');
  assert.deepEqual(errors, []);
  console.log('Browser file selection, cancellation, async reads, clipboard, and delayed paste checks passed.');
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
