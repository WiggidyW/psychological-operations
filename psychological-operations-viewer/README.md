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

## Release

`.github/workflows/release.yml` cuts a release on every push to
`main` when the version in `psychological-operations-cli/Cargo.toml`
doesn't yet have a `v<version>` tag. The same job rejects the
release if `Cargo.toml`, `objectiveai.json`, and this `package.json`
disagree on the version. Bump every file in one move:

```bash
bash version.sh <new-version>
```

The `build-viewer` job runs in parallel with the per-platform
`build-cli` matrix, runs `build.sh`, and uploads the resulting
`psychological-operations-viewer.zip` to the same release. The host
downloads it per `viewer_zip` in `objectiveai.json`.

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
