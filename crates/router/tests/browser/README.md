# Browser history checks

```sh
wasm-pack build crates/router/tests/browser --target web --dev --out-dir pkg --out-name router
npm install --prefix crates/router/tests/browser
cd crates/router/tests/browser
npx playwright install chromium
npm test
```

Checks real browser URL initialization, encoded params, reactive navigation
without reload, replacement, programmatic and native Back/Forward, branching,
direct-path reload, external URL rejection and listener disposal.
