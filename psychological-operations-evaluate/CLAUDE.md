# psychological-operations-evaluate

Evaluation pipelines for `psychological-operations` built on
[ObjectiveAI](https://objectiveai.dev) and
[cocoindex](https://github.com/cocoindex-io/cocoindex).

## Dependencies

Canonical declaration is in `pyproject.toml` `[project.dependencies]`:
```
objectiveai==2.0.1
cocoindex
objectiveai-cocoindex==2.0.1
```
`requirements.txt` mirrors the same pins (kept for callers that
`pip install -r requirements.txt` directly).

`build.sh` installs straight from PyPI — there is no sibling-source
redirect (unlike `objectiveai/objectiveai-cocoindex/build.sh`), since
the `objectiveai*` packages live in the submodule rather than as
sibling crates of this package.

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

## Build

```bash
bash psychological-operations-evaluate/build.sh
```

Creates the venv (if missing) and installs `requirements.txt` + `requirements-dev.txt`.

## Tests

```bash
bash psychological-operations-evaluate/test.sh
bash psychological-operations-evaluate/test.sh -- -k foo -vv
```
