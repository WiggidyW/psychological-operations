import { z } from "zod";
import {
  AgentCompletionsMessageImageUrlSchema,
  AgentCompletionsMessageVideoUrlSchema,
  AgentCompletionsMessageRichContentPartSchema,
} from "objectiveai";
import type { QueuedPost } from "./db.js";

export const PostInputValueSchema = z.object({
  text: z.string(),
  images: z.array(z.object({
    type: z.literal("image_url"),
    image_url: AgentCompletionsMessageImageUrlSchema,
  })),
  videos: z.array(z.object({
    type: z.literal("video_url"),
    video_url: AgentCompletionsMessageVideoUrlSchema,
  })),
});

export type PostInputValue = z.infer<typeof PostInputValueSchema>;

export const PostsInputValueSchema = z.object({
  items: z.array(PostInputValueSchema),
});

export type PostsInputValue = z.infer<typeof PostsInputValueSchema>;

export function newPostInputValue(post: QueuedPost): PostInputValue {
  return {
    text: post.text,
    images: post.images.map((img) => ({ type: "image_url" as const, image_url: img })),
    videos: post.videos.map((vid) => ({ type: "video_url" as const, video_url: vid })),
  };
}
