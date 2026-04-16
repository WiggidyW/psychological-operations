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
