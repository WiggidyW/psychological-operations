import { z } from "zod";
import {
  AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptionalSchema,
  FunctionsFullInlineFunctionOrRemoteCommitOptionalSchema,
  FunctionsInlineProfileOrRemoteCommitOptionalSchema,
  FunctionsExecutionsRequestStrategySchema,
} from "objectiveai";

export const StageSchema = z.object({
  function: FunctionsFullInlineFunctionOrRemoteCommitOptionalSchema,
  profile: FunctionsInlineProfileOrRemoteCommitOptionalSchema,
  strategy: FunctionsExecutionsRequestStrategySchema,
  count: z.number().int().positive().nullable().optional().describe("Number of items to pass into this stage's function."),
  threshold: z.number().min(0).max(1).nullable().optional().describe("Minimum score from the previous stage to enter this stage."),
});

export type Stage = z.infer<typeof StageSchema>;

export const PsyOpSchema = z.object({
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptionalSchema,
  queries: z.array(z.string()),
  count: z.number().int().positive().nullable().optional().describe("Number of top items to notify on."),
  threshold: z.number().min(0).max(1).nullable().optional().describe("Minimum score for notification."),
  max_age: z.number().int().positive().nullable().optional().describe("Maximum age of a post in seconds to be considered."),
  min_likes: z.number().int().nonnegative().nullable().optional().describe("Minimum number of likes for a post to be considered."),
  stages: z.array(StageSchema),
});

export type PsyOp = z.infer<typeof PsyOpSchema>;

export type ValidationResult =
  | { valid: true }
  | { valid: false; reason: "max_age" | "min_likes" };

export function validForPsyop(
  psyop: PsyOp,
  post: { created: string; likes: number },
  now: Date,
): ValidationResult {
  if (psyop.max_age != null) {
    const ageSeconds = (now.getTime() - new Date(post.created).getTime()) / 1000;
    if (ageSeconds > psyop.max_age) return { valid: false, reason: "max_age" };
  }
  if (psyop.min_likes != null) {
    if (post.likes < psyop.min_likes) return { valid: false, reason: "min_likes" };
  }
  return { valid: true };
}
