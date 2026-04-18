use crate::db::QueuedPost;
use crate::input::{new_post_input_value, PostsInputValue};
use crate::psyop::{PsyOp, Stage};

pub struct ScoredPost {
    pub post: QueuedPost,
    pub score: f64,
}

/// Run an objectiveai function execution via the CLI.
/// Returns the parsed output.
fn run_function_execution(
    function_json: &str,
    profile_json: &str,
    input_json: &str,
) -> Result<serde_json::Value, crate::error::Error> {
    let output = std::process::Command::new("objectiveai")
        .args([
            "functions", "executions", "create", "standard",
            "--function-inline", function_json,
            "--profile-inline", profile_json,
            "--input-inline", input_json,
        ])
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
    let result: serde_json::Value = serde_json::from_str(last_line)?;
    Ok(result)
}

pub fn score(psyop: &PsyOp, posts: Vec<QueuedPost>) -> Result<Vec<ScoredPost>, crate::error::Error> {
    let mut current: Vec<ScoredPost> = posts.into_iter()
        .map(|post| ScoredPost { post, score: 0.0 })
        .collect();

    for (i, stage) in psyop.stages.iter().enumerate() {
        eprintln!("Running stage {i} with {} posts...", current.len());

        let items: Vec<_> = current.iter()
            .map(|s| new_post_input_value(&s.post))
            .collect();
        let input = PostsInputValue { items };

        let function_json = serde_json::to_string(&stage.function)?;
        let profile_json = serde_json::to_string(&stage.profile)?;
        let input_json = serde_json::to_string(&input)?;

        let result = run_function_execution(&function_json, &profile_json, &input_json)?;

        // Extract scores
        let output = result.get("output").and_then(|o| o.get("output"))
            .ok_or_else(|| crate::error::Error::Stage { stage: i, message: "missing output".into() })?;

        let scores: Vec<f64> = output.as_array()
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
