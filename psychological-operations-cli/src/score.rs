use objectiveai::functions::{
    FullInlineFunctionOrRemoteCommitOptional,
    FullInlineFunction,
    InlineProfileOrRemoteCommitOptional,
};
use objectiveai::RemotePathCommitOptional;
use serde::Deserialize;

use crate::db::QueuedPost;
use crate::input::{new_post_input_value, PostsInputValue, PostInputValue};
use crate::psyop::{PsyOp, Stage, is_vector_function};

pub struct ScoredPost {
    pub post: QueuedPost,
    pub score: f64,
}

#[derive(Deserialize)]
struct ExecutionOutput {
    output: OutputValue,
}

#[derive(Deserialize)]
struct OutputValue {
    output: serde_json::Value,
}

/// Format a RemotePathCommitOptional for the CLI --path argument.
fn format_remote_ref(path: &RemotePathCommitOptional) -> String {
    serde_json::to_string(path).unwrap()
}

/// Fetch a remote function definition via the CLI and deserialize to inline.
fn fetch_function(path: &RemotePathCommitOptional) -> Result<FullInlineFunction, crate::error::Error> {
    let ref_str = format_remote_ref(path);
    let output = std::process::Command::new("objectiveai")
        .args(["functions", "get", "--path", &ref_str])
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(crate::error::Error::ObjectiveAiCli("failed to fetch function".into()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let function: FullInlineFunction = serde_json::from_str(stdout.trim())?;
    Ok(function)
}

/// Resolve a function to its inline definition, fetching if remote.
fn resolve_function(function: &FullInlineFunctionOrRemoteCommitOptional) -> Result<FullInlineFunction, crate::error::Error> {
    match function {
        FullInlineFunctionOrRemoteCommitOptional::Inline(f) => Ok(f.clone()),
        FullInlineFunctionOrRemoteCommitOptional::Remote(path) => fetch_function(path),
    }
}

/// Run a function execution via the CLI. Always passes inline function and profile.
fn run_function_execution(
    function: &FullInlineFunction,
    profile: &InlineProfileOrRemoteCommitOptional,
    input_json: &str,
    split: bool,
) -> Result<ExecutionOutput, crate::error::Error> {
    let function_json = serde_json::to_string(function)?;
    let profile_json = serde_json::to_string(profile)?;

    let mut args = vec![
        "functions".to_string(), "executions".to_string(), "create".to_string(), "standard".to_string(),
        "--function-inline".to_string(), function_json,
        "--profile-inline".to_string(), profile_json,
        "--input-inline".to_string(), input_json.to_string(),
    ];

    if split {
        args.push("--split".to_string());
    }

    let output = std::process::Command::new("objectiveai")
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
    // Print stdout passthrough (log lines etc.)
    print!("{stdout}");

    // Last line is the JSON result
    let last_line = stdout.trim().lines().last()
        .ok_or_else(|| crate::error::Error::ObjectiveAiCli("no output".into()))?;
    let result: ExecutionOutput = serde_json::from_str(last_line)?;
    Ok(result)
}

pub fn score(psyop: &PsyOp, posts: Vec<QueuedPost>) -> Result<Vec<ScoredPost>, crate::error::Error> {
    let mut current: Vec<ScoredPost> = posts.into_iter()
        .map(|post| ScoredPost { post, score: 0.0 })
        .collect();

    for (i, stage) in psyop.stages.iter().enumerate() {
        eprintln!("Running stage {i} with {} posts...", current.len());

        // Resolve function to inline (fetch if remote)
        let function = resolve_function(&stage.function)?;
        let is_vector = is_vector_function(&function);

        // Build input and execute
        let items: Vec<PostInputValue> = current.iter()
            .map(|s| new_post_input_value(&s.post))
            .collect();

        let (input_json, split) = if is_vector {
            // Vector: wrap in { items: [...] }
            let input = PostsInputValue { items };
            (serde_json::to_string(&input)?, false)
        } else {
            // Scalar: pass as plain array, use --split
            (serde_json::to_string(&items)?, true)
        };

        let result = run_function_execution(&function, &stage.profile, &input_json, split)?;

        // Extract scores
        let scores: Vec<f64> = result.output.output.as_array()
            .ok_or_else(|| crate::error::Error::Stage { stage: i, message: "expected array output".into() })?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect();

        if scores.len() != current.len() {
            return Err(crate::error::Error::Stage {
                stage: i,
                message: format!("score count ({}) doesn't match post count ({})", scores.len(), current.len()),
            });
        }

        for (scored, val) in current.iter_mut().zip(scores.iter()) {
            scored.score = *val;
        }

        current.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Filter for next stage
        if let Some(next_stage) = psyop.stages.get(i + 1) {
            current = filter_for_next_stage(current, next_stage)?;
        }
    }

    Ok(current)
}

fn filter_for_next_stage(mut posts: Vec<ScoredPost>, next_stage: &Stage) -> Result<Vec<ScoredPost>, crate::error::Error> {
    if let Some(threshold) = next_stage.threshold {
        posts.retain(|s| s.score >= threshold);
    }

    if let Some(count) = next_stage.count {
        if next_stage.threshold.is_some() && posts.len() < count as usize {
            return Err(crate::error::Error::Other(format!(
                "not enough posts above threshold {:?} to satisfy count {} (only {} available)",
                next_stage.threshold, count, posts.len(),
            )));
        }
        posts.truncate(count as usize);
    }

    Ok(posts)
}
