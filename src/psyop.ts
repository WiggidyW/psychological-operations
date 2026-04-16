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
  count: z.number().int().positive(),
  threshold: z.number().min(0).max(1),
});

export type Stage = z.infer<typeof StageSchema>;

export const PsyOpSchema = z.object({
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptionalSchema,
  query: z.string(),
  stages: z.array(StageSchema),
});

export type PsyOp = z.infer<typeof PsyOpSchema>;
