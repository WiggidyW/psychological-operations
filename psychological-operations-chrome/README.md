# psychological-operations-chrome

Sister-bundle directory: downloads
[Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/)
for the active Rust target triple and stages the extension so the
parent Rust crate can `include_bytes!` both into a single self-
contained binary.

Mirrors the build pipeline pattern used by
`objectiveai/objectiveai-claude-agent-sdk-runner/` — `build.sh`
produces / refreshes `embed/<target>/<profile>/`, `fingerprint.sh`
hashes the inputs, `validate.sh` is the contract for downstream
`build.rs` consumers.

## Files

- `VERSION` — pinned CfT version (e.g. `131.0.6778.69`). Bumping is
  a one-line change followed by `bash build.sh` to refresh.
- `build.sh` — downloads CfT for the target, stages the extension,
  writes `launch-entry.txt` (relative path to chrome.exe / chrome /
  Chrome.app inside the zip) and `bundle.meta.json`. Logs go to
  `.logs/build/psychological-operations-chrome.txt`.
- `fingerprint.sh` — SHA256 of (VERSION + build.sh + every file in
  `psychological-operations-chrome-extension/`). Source it; sets
  `TARGET`, `PROFILE`, `CHROME_VERSION`, `CFT_PLATFORM`,
  `CHROME_LAUNCH_REL`, `CURRENT_FP`, `FINGERPRINT_FILE`.
- `validate.sh` — exits 0 if `embed/<target>/<profile>/` is fresh
  per the fingerprint, exits 1 if missing, 2 if stale. Called from
  `psychological-operations-cli/build.rs`.

## Output layout

```
embed/<rust-target-triple>/<debug|release>/
├── chrome-bundle.zip          ← CfT zip, copied verbatim
├── extension/                  ← staged extension (will be packed to .crx in a follow-up)
├── launch-entry.txt           ← relative path to the Chromium binary inside the zip
├── bundle.meta.json           ← provenance (URL, version, platform, byte count)
└── .fingerprint
```

The whole `embed/` tree is gitignored — these are large binary
artifacts produced from the pinned version + extension sources.

## Usage

```sh
bash psychological-operations-chrome/build.sh                   # host target, debug
bash psychological-operations-chrome/build.sh --release         # host target, release
bash psychological-operations-chrome/build.sh --target x86_64-unknown-linux-gnu  # cross
```

Re-runs are no-ops via the fingerprint short-circuit unless the
extension files, the pinned chrome version, or the build script
itself have changed.

## Target → Chrome for Testing platform

| Rust target                                           | CfT platform | Launch entry                                                                              |
| ----------------------------------------------------- | ------------ | ----------------------------------------------------------------------------------------- |
| `x86_64-pc-windows-msvc` / `x86_64-pc-windows-gnu`    | `win64`      | `chrome-win64/chrome.exe`                                                                 |
| `i686-pc-windows-msvc` / `i686-pc-windows-gnu`        | `win32`      | `chrome-win32/chrome.exe`                                                                 |
| `x86_64-unknown-linux-gnu` / `x86_64-unknown-linux-musl` | `linux64` | `chrome-linux64/chrome`                                                                   |
| `aarch64-apple-darwin`                                | `mac-arm64`  | `chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing` |
| `x86_64-apple-darwin`                                 | `mac-x64`    | `chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`   |

## Coming in follow-up commits

1. **`crx-pack/`** — small Rust binary that packs the extension dir
   into a signed `.crx` using a committed RSA-2048 PKCS#8 key
   (`extension-key.pem`, also added in the next commit). Output
   replaces the staged `extension/` directory above.
2. **`extension-key.pem`** — committed RSA key. Public-key portion
   also pasted into the extension's `manifest.json` `key` field for
   a deterministic extension ID across all installs and machines.
3. **`psychological-operations-cli/build.rs`** — calls
   `validate.sh`, exposes `embed/<target>/<profile>/` paths to the
   `chrome` Rust module via `cargo:rustc-env=…`.
4. **`psychological-operations-cli/src/chrome/`** — `include_bytes!`
   the chrome zip + extension.crx, content-hash-extract on first
   launch, force-install policy, native-host registration, spawn.
5. **`browse <psyop>` subcommand** — entry point that ties it all
   together.
