# psychological-operations-viewer

Plugin viewer that the [`objectiveai`](https://github.com/ObjectiveAI/objectiveai)
host renders inside its sandboxed plugin iframe.

The build output is zipped into `psychological-operations-viewer.zip`
at the repo root and uploaded as a release asset — the host downloads
it per the `viewer_zip` field in `objectiveai.json` and serves the
bundle at `plugin://localhost/psychological-operations/...`.

## Dev

```bash
pnpm install
pnpm dev          # vite dev server, http://localhost:5173
```

In dev the bundle runs at the regular origin (`localhost`), so
`@objectiveai/sdk/viewer`'s `listen()` falls through to a Tauri-style
event source if available. Inbound events come from the host
process only when running embedded — for standalone iteration on UI
shape, use static mocks.

## Build

```bash
pnpm build        # tsc check + vite build → dist/
bash build.sh     # the above + zip dist/ → ../psychological-operations-viewer.zip
```

## Sandbox constraints

The host iframe is `sandbox="allow-scripts allow-forms"` —
**no `allow-same-origin`**, which means:

- No `localStorage` / `sessionStorage` / IndexedDB.
- No service workers.
- No `fetch()` to the plugin's own origin (the `plugin://` scheme is
  treated as opaque).
- Any state that needs to round-trip with the plugin process flows
  through `@objectiveai/sdk/viewer`'s `listen()` (host → iframe)
  and any host-side HTTP routes the plugin author declares via
  `viewer_routes` in `objectiveai.json`.
