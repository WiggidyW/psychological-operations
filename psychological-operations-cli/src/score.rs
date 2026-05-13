use objectiveai::functions::{
    FullInlineFunctionOrRemoteCommitOptional,
    FullInlineFunction,
    InlineProfileOrRemoteCommitOptional,
};
use objectiveai::functions::executions::request::Strategy;
use objectiveai::RemotePathCommitOptional;
use serde::Deserialize;

use crate::db::Post;
use crate::input::{new_post_input_value, PostsInputValue, PostInputValue};
use crate::psyops::{Stage, is_vector_function};

#[derive(Clone)]
pub struct ScoredPost {
    pub post: Post,
    pub score: f64,
}

#[derive(Deserialize)]
struct ExecutionOutput {
    output: serde_json::Value,
}

/// Format a RemotePathCommitOptional for the CLI --path argument.
/// The CLI expects `key=value,key=value` format, not JSON.
fn format_remote_ref(path: &RemotePathCommitOptional) -> String {
    match path {
        RemotePathCommitOptional::Github { owner, repository, commit } => {
            let mut s = format!("remote=github,owner={owner},repository={repository}");
            if let Some(c) = commit {
                s.push_str(&format!(",commit={c}"));
            }
            s
        }
        RemotePathCommitOptional::Filesystem { owner, repository, commit } => {
            let mut s = format!("remote=filesystem,owner={owner},repository={repository}");
            if let Some(c) = commit {
                s.push_str(&format!(",commit={c}"));
            }
            s
        }
        RemotePathCommitOptional::Mock { name } => {
            format!("remote=mock,name={name}")
        }
    }
}

/// Locate the objectiveai CLI used for subprocess scoring calls
/// (`functions get`, `executions create`, …).
///
/// Resolution order:
///   1. `PSYCHOLOGICAL_OPERATIONS_OBJECTIVEAI_BINARY` env var
///      — explicit override. The integration-test harness uses this
///      to point at a no-viewer release binary so test runs don't
///      spawn viewer windows.
///   2. `<HOME|USERPROFILE>/.objectiveai/objectiveai[.exe]` — the
///      install script's default location. The Windows installer
///      only updates user-env PATH, which isn't reflected in
///      already-running shells, so this fallback is what makes the
///      bare `objectiveai` invocation work after a fresh install.
///   3. `PATH` lookup (last-resort bare name).
pub fn objectiveai_binary(_cfg: &crate::run::Config) -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Ok(p) = std::env::var("PSYCHOLOGICAL_OPERATIONS_OBJECTIVEAI_BINARY") {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) { "objectiveai.exe" } else { "objectiveai" };
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let candidate = PathBuf::from(home).join(".objectiveai").join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(name)
}

/// Scan a JSONL stdout stream and return the `value` of the LAST
/// `notification` line whose `value` contains the requested top-level
/// `key`. The post-2.0.4 objectiveai CLI emits multiple notifications
/// per command: incremental progress (e.g. `log_stream_ready`) plus
/// the final terminal payload (e.g. `execution`, `function`,
/// `instructions`). Picking by key lets each caller wait for its
/// specific terminal notification.
///
/// An `error` line short-circuits with the host's error message
/// regardless of position.
fn parse_notification_value(stdout: &str, key: &str) -> Result<serde_json::Value, crate::error::Error> {
    let mut last_match: Option<serde_json::Value> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue; // skip non-JSON noise (warnings, panics)
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("notification") => {
                if let Some(value) = v.get("value") {
                    if value.get(key).is_some() {
                        last_match = Some(value.clone());
                    }
                }
            }
            Some("error") => {
                let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("(no message)");
                return Err(crate::error::Error::ObjectiveAiCli(msg.to_string()));
            }
            _ => {}
        }
    }
    last_match.ok_or_else(|| crate::error::Error::ObjectiveAiCli(
        format!("no `notification` with field `{key}` in CLI stdout: {stdout}"),
    ))
}

/// Fetch a remote function definition via the CLI and deserialize to inline.
///
/// The CLI now wraps `functions get` output as:
///   `{"type":"notification","value":{"function":<GetFunctionResponse>}}`
/// where `GetFunctionResponse` flattens `RemotePath` (remote/owner/…)
/// alongside the function body (`type`, `description`, `input_schema`,
/// `tasks`). We pluck the `function` field, drop the path metadata by
/// deserializing as `FullInlineFunction` (serde ignores unknown fields),
/// and rely on `InlineFunction` accepting the extra `description` /
/// `input_schema` keys without complaint.
fn fetch_function(path: &RemotePathCommitOptional, cfg: &crate::run::Config) -> Result<FullInlineFunction, crate::error::Error> {
    let ref_str = format_remote_ref(path);
    let output = std::process::Command::new(objectiveai_binary(cfg))
        .args(["functions", "get", "--path", &ref_str])
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(crate::error::Error::ObjectiveAiCli("failed to fetch function".into()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = parse_notification_value(&stdout, "function")?;
    let function_value = &value["function"];
    let function: FullInlineFunction = serde_json::from_value(function_value.clone())?;
    Ok(function)
}

/// Resolve a function to its inline definition, fetching if remote.
fn resolve_function(function: &FullInlineFunctionOrRemoteCommitOptional, cfg: &crate::run::Config) -> Result<FullInlineFunction, crate::error::Error> {
    match function {
        FullInlineFunctionOrRemoteCommitOptional::Inline(f) => Ok(f.clone()),
        FullInlineFunctionOrRemoteCommitOptional::Remote(path) => fetch_function(path, cfg),
    }
}

/// Fetch a fresh function-execution `--instructions-id` from the CLI. The
/// objectiveai CLI requires this token on every `executions create` call;
/// it ties a specific execution to the current instructions revision.
///
/// Wire: `{"type":"notification","value":{"instructions":"<markdown>"}}`.
/// The markdown body ends with an `Instructions ID: <id>` line — extract
/// that ID and return it.
fn fetch_instructions_id(cfg: &crate::run::Config) -> Result<String, crate::error::Error> {
    let output = std::process::Command::new(objectiveai_binary(cfg))
        .args(["functions", "executions", "instructions", "get"])
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(crate::error::Error::ObjectiveAiCli(
            format!("instructions get failed: {}", String::from_utf8_lossy(&output.stdout)),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = parse_notification_value(&stdout, "instructions")?;
    let instructions = value["instructions"].as_str().ok_or_else(|| {
        crate::error::Error::ObjectiveAiCli(
            format!("instructions get notification `instructions` field is not a string: {value}"),
        )
    })?;
    let id = instructions.lines().rev()
        .find_map(|l| l.trim().strip_prefix("Instructions ID:").map(|s| s.trim().to_string()))
        .ok_or_else(|| crate::error::Error::ObjectiveAiCli(
            format!("instructions body missing `Instructions ID:` line: {instructions}"),
        ))?;
    Ok(id)
}

/// Run a function execution via the CLI. Dispatches to either the
/// `standard` or `swiss-system` subcommand based on the psyop's strategy,
/// fetches a fresh `--instructions-id` for the call, and always passes the
/// inline function + profile.
fn run_function_execution(
    function: &FullInlineFunction,
    profile: &InlineProfileOrRemoteCommitOptional,
    strategy: &Strategy,
    input_json: &str,
    split: bool,
    invert: bool,
    seed: Option<i64>,
    cfg: &crate::run::Config,
) -> Result<ExecutionOutput, crate::error::Error> {
    let function_json = serde_json::to_string(function)?;
    let profile_json = serde_json::to_string(profile)?;
    let instructions_id = fetch_instructions_id(cfg)?;

    let subcommand = match strategy {
        Strategy::Default => "standard",
        Strategy::SwissSystem { .. } => "swiss-system",
    };

    let mut args = vec![
        "functions".to_string(), "executions".to_string(), "create".to_string(), subcommand.to_string(),
        "--instructions-id".to_string(), instructions_id,
        "--function-inline".to_string(), function_json,
        "--profile-inline".to_string(), profile_json,
        "--input-inline".to_string(), input_json.to_string(),
    ];

    if let Strategy::SwissSystem { pool, rounds } = strategy {
        if let Some(p) = pool {
            args.push("--pool".to_string());
            args.push(p.to_string());
        }
        if let Some(r) = rounds {
            args.push("--rounds".to_string());
            args.push(r.to_string());
        }
    }

    if split {
        args.push("--split".to_string());
    }
    if invert {
        args.push("--invert".to_string());
    }
    if let Some(s) = seed {
        args.push("--seed".to_string());
        args.push(s.to_string());
    }

    let output = std::process::Command::new(objectiveai_binary(cfg))
        .args(&args)
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(crate::error::Error::ObjectiveAiCli(
            String::from_utf8_lossy(&output.stdout).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // NOTE: we do NOT echo subprocess stdout — the host treats *our*
    // stdout as a JSONL stream, and forwarding the inner CLI's JSONL
    // verbatim would duplicate `begin`/`end` markers and confuse the
    // host. Anything that needs to surface to the user goes through
    // stderr or our own structured emit.

    // Post-2.0.4 wire shape: the CLI emits multiple notifications per
    // execution (progress markers like `log_stream_ready`, then the
    // terminal `execution` payload). Pick the last one whose value
    // carries `execution`. Wire:
    //   {"type":"notification","value":{"execution":{"output":...,"errors":[...]}}}
    let value = parse_notification_value(&stdout, "execution")?;
    let execution = &value["execution"];
    let errors = execution.get("errors").and_then(|e| e.as_array()).cloned().unwrap_or_default();
    if !errors.is_empty() {
        // Mirror the previous warning behavior: silent fallback to a
        // uniform prior. Keep the message byte-stable across runs by
        // omitting any per-execution IDs.
        eprintln!(
            "warning: objectiveai execution reported {} task error(s) \
             — output may be a fallback",
            errors.len(),
        );
    }
    let inner = execution.get("output").cloned().ok_or_else(|| {
        crate::error::Error::ObjectiveAiCli(
            format!("executions create notification missing `execution.output`: {value}"),
        )
    })?;

    Ok(ExecutionOutput { output: inner })
}

/// Run a single stage's function execution against the given posts.
/// Returns scored posts in score-descending order.
pub fn score(stage: &Stage, posts: Vec<Post>, seed: Option<i64>, cfg: &crate::run::Config) -> Result<Vec<ScoredPost>, crate::error::Error> {
    let mut scored: Vec<ScoredPost> = posts.into_iter()
        .map(|p| ScoredPost { post: p, score: 0.0 })
        .collect();

    eprintln!("Scoring {} posts...", scored.len());

    let function = resolve_function(&stage.function, cfg)?;
    let is_vector = is_vector_function(&function);

    let items: Vec<PostInputValue> = scored.iter()
        .map(|s| new_post_input_value(&s.post, stage.images, stage.videos))
        .collect();

    let (input_json, split) = if is_vector {
        let input = PostsInputValue { items };
        (serde_json::to_string(&input)?, false)
    } else {
        (serde_json::to_string(&items)?, true)
    };

    let result = run_function_execution(&function, &stage.profile, &stage.strategy, &input_json, split, stage.invert, seed, cfg)?;

    let scores: Vec<f64> = result.output.as_array()
        .ok_or_else(|| crate::error::Error::Other("expected array output".into()))?
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0))
        .collect();

    if scores.len() != scored.len() {
        return Err(crate::error::Error::Other(
            format!("score count ({}) doesn't match post count ({})", scores.len(), scored.len()),
        ));
    }

    for (s, val) in scored.iter_mut().zip(scores.iter()) {
        s.score = *val;
    }

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(scored)
}
