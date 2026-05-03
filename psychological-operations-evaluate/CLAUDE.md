# psychological-operations-evaluate

PyInstaller-bundled runner that packages
[`objectiveai`](https://objectiveai.dev),
[`cocoindex`](https://github.com/cocoindex-io/cocoindex), and
[`objectiveai-cocoindex`](https://github.com/ObjectiveAI/objectiveai)
(plus their transitive deps) into a single self-contained executable
per target/profile. Mirrors the build pipeline of
`objectiveai/objectiveai-claude-agent-sdk-runner/`.

## Build

```bash
bash psychological-operations-evaluate/build.sh                          # debug, host target
bash psychological-operations-evaluate/build.sh --release                # release, host target
bash psychological-operations-evaluate/build.sh --target <triple>        # cross-target metadata only
```

`build.sh` is fingerprint-driven: if `main.py`, `requirements.txt`, and
`requirements-dev.txt` are all unchanged from the last successful
build, PyInstaller is skipped. Logs land in
`.logs/build/psychological-operations-evaluate.txt`.

## Output

```
psychological-operations-evaluate/embed/<target>/<profile>/
├── .fingerprint
└── psychological-operations-evaluate[.exe]
```

`<target>` is the Rust target triple (auto-detected via `rustc -vV`
unless `--target` is passed). `<profile>` is `debug` or `release`.

## Validate

```bash
bash psychological-operations-evaluate/validate.sh [--release] [--target <triple>]
```

Exits 0 if `embed/<target>/<profile>/` is present and its fingerprint
matches the current source. Exits 1 if the embed directory is missing
and 2 if the fingerprint is stale. Designed for use by a future Rust
`build.rs` consumer (see `psychological-operations-cli/build.rs` for
the playwright equivalent).

## Virtual Environment

**CRITICAL: Never run bare `python` or `pip` commands.** Always use the venv:

```bash
# Windows
psychological-operations-evaluate/venv/Scripts/python.exe <args>
psychological-operations-evaluate/venv/Scripts/pip.exe <args>

# Linux/macOS
psychological-operations-evaluate/venv/bin/python <args>
psychological-operations-evaluate/venv/bin/pip <args>
```
