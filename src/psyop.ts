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
  count: z.number().int().positive().describe("Number of items to pass into this stage's function."),
  threshold: z.number().min(0).max(1).describe("Minimum score from the previous stage to enter this stage."),
});

export type Stage = z.infer<typeof StageSchema>;

export const PsyOpSchema = z.object({
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptionalSchema,
  query: z.string(),
  count: z.number().int().positive().describe("Number of top items to notify on."),
  threshold: z.number().min(0).max(1).describe("Minimum score for notification."),
  stages: z.array(StageSchema),
});

export type PsyOp = z.infer<typeof PsyOpSchema>;
