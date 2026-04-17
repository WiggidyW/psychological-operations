import {
  ObjectiveAI,
  functionsExecutionsCreateFunctionExecution,
} from "objectiveai";
import type { QueuedPost } from "./db.js";
import type { PsyOp, Stage } from "./psyop.js";
import { newPostInputValue } from "./input.js";
import type { PostInputValue, PostsInputValue } from "./input.js";

interface ScoredPost {
  post: QueuedPost;
  score: number;
}

/**
 * Run all stages of a psyop against the given posts.
 *
 * Each stage runs a vector function execution. The output is a score per post.
 * Between stages, posts are filtered and narrowed by threshold and count.
 *
 * Returns the final set of scored posts after all stages.
 */
export async function score(
  client: ObjectiveAI,
  psyop: PsyOp,
  posts: QueuedPost[],
): Promise<ScoredPost[]> {
  let current: ScoredPost[] = posts.map((post) => ({ post, score: 0 }));

  for (let i = 0; i < psyop.stages.length; i++) {
    const stage = psyop.stages[i];
    if (stage === undefined) break;
    console.log(`Running stage ${i} with ${current.length} posts...`);

    // Build input
    const items: PostInputValue[] = current.map((s) => newPostInputValue(s.post));
    const input: PostsInputValue = { items };

    // Execute function
    const result = await functionsExecutionsCreateFunctionExecution(client, {
      function: stage.function,
      profile: stage.profile,
      strategy: stage.strategy,
      input,
    });

    // Extract scores — vector function returns number[]
    const output = result.output.output;
    if (!Array.isArray(output) || typeof output[0] !== "number") {
      throw new Error(`Stage ${i}: expected vector output (number[]), got ${typeof output}`);
    }
    const scores = output as number[];

    if (scores.length !== current.length) {
      throw new Error(`Stage ${i}: score count (${scores.length}) doesn't match post count (${current.length})`);
    }

    // Assign scores
    current = current.map((s, j) => {
      const sc = scores[j];
      if (sc === undefined) throw new Error(`Stage ${i}: missing score at index ${j}`);
      return { ...s, score: sc };
    });

    // Sort by score descending
    current.sort((a, b) => b.score - a.score);

    // Apply filtering for next stage (skip on last stage)
    const nextStage = psyop.stages[i + 1];
    if (nextStage !== undefined) {
      current = filterForNextStage(current, nextStage);
    }
  }

  return current;
}

/**
 * Filter scored posts for the next stage based on its threshold and count.
 * Threshold takes priority over count — if both are specified and there
 * aren't enough posts above the threshold to satisfy count, we exit early.
 */
function filterForNextStage(posts: ScoredPost[], nextStage: Stage): ScoredPost[] {
  let filtered = posts;

  // Apply threshold first (priority)
  const threshold = nextStage.threshold;
  if (threshold != null) {
    filtered = filtered.filter((s) => s.score >= threshold);
  }

  // Apply count
  if (nextStage.count != null) {
    if (threshold != null && filtered.length < nextStage.count) {
      throw new Error(
        `Not enough posts above threshold ${threshold} to satisfy count ${nextStage.count} ` +
        `(only ${filtered.length} available)`,
      );
    }
    filtered = filtered.slice(0, nextStage.count);
  }

  return filtered;
}
