import { z } from "zod";
import {
  AgentCompletionsMessageImageUrlSchema,
  AgentCompletionsMessageVideoUrlSchema,
} from "objectiveai";
import type { QueuedPost } from "./db.js";

export const PostInputValueSchema = z.object({
  text: z.string(),
  images: z.array(AgentCompletionsMessageImageUrlSchema),
  videos: z.array(AgentCompletionsMessageVideoUrlSchema),
});

export type PostInputValue = z.infer<typeof PostInputValueSchema>;

export const PostsInputValueSchema = z.object({
  items: z.array(PostInputValueSchema),
});

export type PostsInputValue = z.infer<typeof PostsInputValueSchema>;

export function newPostInputValue(post: QueuedPost): PostInputValue {
  return { text: post.text, images: post.images, videos: post.videos };
}
