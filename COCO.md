# COCO — A generic ObjectiveAI library for CocoIndex

This document outlines how a generic `cocoindex-objectiveai` library should work,
and how it would be used to integrate ObjectiveAI scoring into `psychological-operations`.

It is a design sketch, not a finished implementation.

## Background

### What psychological-operations does today

`psychological-operations` is an agentic X scraper + scorer.

- **Scrape**: a Playwright-driven agent searches X for tweets matching configured queries
  and stores them in a local SQLite DB (`posts`, `post_contents`, `post_tags`, `scores`,
  `score_tags`). Every scraped tweet is tagged.
- **Score**: a *psyop* (JSON definition under `psyops/<name>/psyop.json`) declares:
  - `sources`: a list of `{tag, min_likes?, min_age?, max_age?, min_score?, count?, …}`
    filters that select candidate posts from the DB by tag.
  - `function`: an ObjectiveAI **Function** reference (inline JSON or
    `{remote: github|filesystem|mock, owner, repository, commit}`).
  - `profile`: an ObjectiveAI **Profile** reference (same shape — learned weights).
  - `strategy`: `Default` or `SwissSystem { pool, rounds }`.
  - `tags`: tags written onto every score row this psyop produces. Other psyops
    can then read these via their `Source.tag` + `min_score`.
- **Run**: `psyops run` shells out to the `objectiveai` CLI
  (`functions executions create standard|swiss-system …`) per psyop, parses the
  resulting score vector, and persists it to `scores` / `score_tags`.

Two function shapes are supported:

- **Vector function** — input is a *batch* of items; output is a vector of scores
  that sums to ≈ 1. Used for *relative* ranking. The whole batch is one
  execution; you cannot score one tweet in isolation.
- **Scalar function** — input is a single item; output is a single score in
  `[0, 1]`. The CLI is invoked with `--split` so many items in one call become
  many independent executions.

Cascading psyops (cheap pass → expensive re-rank of the top N%) are expressed
today by tagging the cheap psyop's output and pointing the expensive psyop's
`sources[].tag` at it (with a `min_score` threshold).

### What CocoIndex gives us

CocoIndex is a declarative, state-driven incremental data pipeline framework.
You write Python that *declares* what should exist in a target as a function of
the current source state; CocoIndex diffs against the previous run and applies
the minimum set of inserts / updates / deletes.

Concepts we need:

- **App** — the top-level runnable. `coco.App(name, main_fn, **args)`.
- **Processing component** — a unit of incremental execution mounted at a
  stable component path (`coco.component_subpath(...)`). Each component owns a
  set of target states; CocoIndex syncs them as a unit when the component
  finishes, and cleans them up if the component is no longer mounted next run.
- **`@coco.fn`** — decorates a Python function so its calls participate in
  change detection. With `memo=True` the result is cached and skipped when
  inputs and logic are unchanged.
- **`deps=`** — declares external values (prompts, model IDs, …) as part of the
  function's logic fingerprint. Bumping `deps` invalidates dependent memos.
- **Batching** — `@coco.fn.as_async(batching=True, max_batch_size=N)` collapses
  many concurrent `await fn(x)` calls into one `fn([x1, x2, …])` invocation
  that returns the corresponding list of results.
- **Mounting APIs** — `mount()`, `use_mount()`, `mount_each(fn, items.items())`,
  `mount_target(...)`, `map(fn, items)`. `mount_each` is the common case for
  fanning out one component per source item.
- **Targets** — connectors expose `declare_*` / `mount_*_target` to declare
  desired rows / files / vectors in external systems.

The crucial property: CocoIndex is the right substrate for psychological-operations
because *the entire pipeline is incremental by nature*. New tweets arrive; old
tweets get re-ranked when a profile commit moves; deleted tweets should drop
their scores. State-driven sync gets that for free, instead of being hand-rolled
inside `db.rs`.

## Mapping ObjectiveAI primitives to CocoIndex

The library exposes two primitives, each as a `@coco.fn`-decorated callable:

### Scalar — per-item independent scoring

A scalar function maps **one item → one score**. Calls are independent, so:

- The natural mounting unit is **per item**.
- Memoization works *per item*: the cache key is `(function_commit, profile_commit, item)`.
- Many concurrent calls should batch into a single `--split` execution to amortize
  request overhead. CocoIndex's built-in batching is exactly this.

### Vector — whole-batch relative ranking

A vector function maps **a batch of items → a score vector summing to ≈ 1**.
Each score depends on the rest of the batch, so:

- The natural mounting unit is **per batch**, *not* per item.
- The memoization key is the *whole input set* (canonicalized — order-independent
  if the user wants, otherwise order-sensitive).
- Batching across calls is meaningless: a vector function call *is* the batch.
  Concurrency happens *across* psyops / batches, not within one.

This split — scalar = per-item, vector = per-batch — is the central design
constraint and drives every API choice below.

## Library API

Package: `cocoindex-objectiveai`. Public surface lives in
`cocoindex_objectiveai/__init__.py` and re-exports the pieces below.

### Reference types

Mirror the existing Rust enums (`FullInlineFunctionOrRemoteCommitOptional`,
`InlineProfileOrRemoteCommitOptional`) one-to-one as Pydantic models — so
psyop JSON files round-trip:

```python
class FunctionRef(BaseModel):
    @classmethod
    def github(cls, owner: str, repository: str, commit: str | None = None) -> "FunctionRef": ...
    @classmethod
    def filesystem(cls, owner: str, repository: str, commit: str | None = None) -> "FunctionRef": ...
    @classmethod
    def mock(cls, name: str) -> "FunctionRef": ...
    @classmethod
    def inline(cls, definition: dict) -> "FunctionRef": ...

class ProfileRef(BaseModel): ...   # same shape

class Strategy(BaseModel):
    @classmethod
    def default(cls) -> "Strategy": ...
    @classmethod
    def swiss_system(cls, *, pool: int | None = None, rounds: int | None = None) -> "Strategy": ...
```

Both refs implement `__coco_memo_key__()` so CocoIndex can fingerprint them
cheaply. Crucially, a remote ref *with* a commit SHA fingerprints to the SHA;
*without* a commit, it fingerprints to a pinned-at-call-time SHA the library
resolves on first use (so floating refs still memoize correctly within a run).

### Client

```python
class ObjectiveAI:
    def __init__(
        self,
        *,
        api_key: str | None = None,
        base_url: str = "https://api.objectiveai.dev",
        binary: str | None = None,            # path to objectiveai CLI; if set, shell out
    ): ...

    def scalar(
        self,
        *,
        function: FunctionRef,
        profile: ProfileRef,
        invert: bool = False,
        max_batch_size: int = 32,
    ) -> "ScalarFunction": ...

    def vector(
        self,
        *,
        function: FunctionRef,
        profile: ProfileRef,
        strategy: Strategy = Strategy.default(),
        invert: bool = False,
    ) -> "VectorFunction": ...
```

Two backends, picked at construction time:

- **HTTP** (default) — uses `objectiveai-py`'s async client directly:
  `create_function_execution(client, params)`. Streaming optional.
- **CLI shell-out** — wraps the existing `objectiveai functions executions
  create …` flow. Useful for parity with today's `score.rs`. Selected by
  passing `binary=`.

The two backends are interchangeable behind the `ScalarFunction` / `VectorFunction`
interfaces.

### ScalarFunction

```python
class ScalarFunction:
    function: FunctionRef
    profile: ProfileRef
    invert: bool

    @coco.fn(memo=True, deps={...self...})
    async def score(self, item: Any) -> float:
        """Score one item. Concurrent calls auto-batch into one --split execution."""
```

Implementation sketch:

- `score` is built once per `ScalarFunction` instance via
  `coco.fn.as_async(batching=True, max_batch_size=self.max_batch_size,
  deps={"function": function, "profile": profile, "invert": invert})`.
- The batched implementation receives `items: list[Any]`, makes one call to
  `functions/executions` with `--split`, and returns `list[float]` aligned to
  the input order.
- `deps=` carries the fingerprint of the function and profile refs, so any
  commit bump or profile change invalidates downstream memos automatically.

### VectorFunction

```python
class VectorFunction:
    function: FunctionRef
    profile: ProfileRef
    strategy: Strategy
    invert: bool

    @coco.fn(memo=True, deps={...self...})
    async def rank(self, items: Sequence[Any]) -> list[float]:
        """Rank a whole batch. Memoized on the input set as a unit."""
```

Implementation sketch:

- `items` is canonicalized (tuple) before fingerprinting; user supplies a
  `key=` hook if items aren't natively hashable.
- One call to `functions/executions` per `rank()` invocation. No batching.
- For `Strategy.swiss_system(pool=…, rounds=…)`, the call is dispatched to the
  `swiss-system` subcommand / endpoint.
- The result is a `list[float]` summing to ≈ 1, in input order.

### Cascade helper

Exactly the cheap-pass / expensive-pass psyop pattern, factored out:

```python
async def cascade(
    items: Sequence[T],
    *,
    cheap: VectorFunction,
    expensive: VectorFunction,
    cheap_keep_top: int | float,        # int = top-N; float in (0,1] = top fraction
) -> list[tuple[T, float]]:
    """Rank with `cheap`, keep the top slice, re-rank that slice with `expensive`."""
```

This is the highest-leverage convenience: it's the literal control flow of
every interesting psyop today, and it composes naturally with cocoindex
mounting because the inner `cheap` and `expensive` calls are themselves
memoized `@coco.fn`s.

### Profile improvement (later)

Profiles are learnable (the README mentions it). A future addition:

```python
profile = await oai.improve_profile(
    function=...,
    base_profile=...,
    feedback=[(item, label_in_0_1), ...],
)
```

This is a separate API endpoint and orthogonal to scoring. Left out of v1.

## Example: a psyop expressed in CocoIndex

A two-pass ranking flow that mirrors today's `psyops run`:

```python
import pathlib
import cocoindex as coco
from cocoindex.connectors import sqlite          # hypothetical: existing SQLite connector
from cocoindex_objectiveai import (
    ObjectiveAI, FunctionRef, ProfileRef, Strategy, cascade,
)

oai = ObjectiveAI()

CHEAP = oai.vector(
    function=FunctionRef.github("me", "psyop-fn", commit="abc123"),
    profile=ProfileRef.github("me", "psyop-fn", commit="abc123"),
)
EXPENSIVE = oai.vector(
    function=FunctionRef.github("me", "psyop-fn", commit="abc123"),
    profile=ProfileRef.github("me", "psyop-fn-expensive", commit="def456"),
    strategy=Strategy.swiss_system(pool=8, rounds=4),
)

@coco.fn(memo=True)
async def score_batch(
    posts: tuple[Post, ...],            # tuple ⇒ stable, hashable memo key
    table: sqlite.TableTarget[Score],
) -> None:
    ranked = await cascade(posts, cheap=CHEAP, expensive=EXPENSIVE, cheap_keep_top=0.20)
    for post, score in ranked:
        table.declare_row(Score(post_id=post.id, psyop="my-psyop", score=score))

@coco.fn
async def app_main(db_path: pathlib.Path) -> None:
    posts = sqlite.query(db_path, "SELECT … FROM posts JOIN post_tags … WHERE tag = ?", "my-tag")
    table = await sqlite.mount_table_target(db_path, "scores", schema=Score)
    # Group posts into rank-able batches, mount one component per batch.
    for batch_id, batch in batch_by_query(posts):
        await coco.mount(
            coco.component_subpath("rank", batch_id),
            score_batch, tuple(batch), table,
        )

app = coco.App("psyop:my-psyop", app_main, db_path=pathlib.Path("./posts.db"))
```

What CocoIndex gives us automatically:

- **New tweet appears** under tag `my-tag` → only its batch's component
  re-runs; other batches' scores stay cached.
- **Tweet is deleted** from the source → its batch re-ranks without it; the
  score row for the deleted tweet is dropped from `scores` because no
  component declares it anymore.
- **Profile commit bumps** (`def456` → `def789`) → `EXPENSIVE`'s deps
  fingerprint changes → every batch invalidates and re-ranks. The cheap pass
  cache is untouched.
- **Cheap function commit bumps** → cheap pass cache invalidates everywhere.
- **`cocoindex update` is idempotent** — re-running with no changes is a no-op.

## Integration into psychological-operations

The current architecture is:

```
psychological-operations-cli (Rust)
  ├── score.rs        → spawns objectiveai CLI per psyop
  ├── db.rs           → SQLite reads/writes (posts, scores, tags)
  └── psyops/run.rs   → orchestration: select sources, dedupe, score, persist
```

Migration plan, smallest-step-first:

1. **Build `cocoindex-objectiveai` standalone** — the library above, with a
   pytest suite that uses the `mock` remote and a fake CLI.
2. **Mirror the SQLite schema as a CocoIndex source/target connector** —
   either reuse an existing `cocoindex.connectors.sqlite` if present, or write
   a minimal one targeting the existing `posts` / `scores` tables. The Rust
   CLI keeps writing `posts`; CocoIndex reads from it.
3. **Reimplement `psyops run` as a CocoIndex App** — one `app_main` per psyop,
   parameterized by the existing `psyop.json`. Translation:
   - `sources[].tag` + filters → SQL/predicate that builds the input set.
   - `function` / `profile` → `FunctionRef` / `ProfileRef`.
   - `strategy` → `Strategy`.
   - `tags` → tags written into `score_tags` via target-state declarations.
4. **Replace `score.rs`'s shell-out** — the Rust CLI invokes `cocoindex update
   psyops/<name>/main.py` instead of running the whole loop itself. (Or, more
   gradually, the Rust CLI keeps running the legacy path while a feature flag
   routes specific psyops through the new pipeline.)
5. **Cascading psyops compose for free** — psyop B's `sources[].tag` reading
   from psyop A's output simply means psyop B's CocoIndex App reads rows from
   the `scores` table where psyop A wrote them, with the right `min_score`
   predicate.

The Rust scrape side (Playwright harness, agent intervention, notifications)
stays as-is. Only the score / cascade / persist path moves into CocoIndex.

### Why bother

- **Fewer rebuilds**: today, re-running a psyop after a profile bump rescues
  nothing — the entire batch is recomputed. With CocoIndex, only the affected
  pass re-runs.
- **Cheaper iteration**: tweaking a cheap-pass profile while leaving the
  expensive pass alone preserves the expensive pass's cache (assuming the
  cheap top-N% is unchanged). Today every iteration burns full ObjectiveAI
  credits.
- **Live mode**: `cocoindex update --live` watches the SQLite source for new
  rows and rescore continuously, replacing the current "run on cron" loop.
- **Lineage**: every score row is traceable to its inputs (post snapshot,
  function commit, profile commit) via CocoIndex's tracking, satisfying the
  audit story we'd otherwise have to build by hand.

## Open questions

- **CLI vs HTTP** — do we want both backends, or pick one? CLI shell-out
  preserves bug-for-bug parity with today; HTTP is faster and cleaner.
  Recommend HTTP with a CLI escape hatch for ops that prefer to debug with the
  CLI.
- **Memoization key for vector functions** — order-independent (treat `items`
  as a set) or order-dependent? Order-independent is more cache-friendly but
  surprises users whose inputs are intentionally ordered. Recommend
  order-dependent by default with an explicit `unordered=True` opt-in.
- **Streaming executions** — vector functions can stream partial scores. Do we
  expose that, or always block until completion? Recommend completion-only for
  v1; streaming is a v2 feature with its own incremental-target story.
- **Function-invention endpoint** — psyop creation today calls
  `objectiveai functions invent`. Worth wrapping in the library, but not
  needed for the core scoring path. Defer.
