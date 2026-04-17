const MAX_CONTENT_LENGTH = 2000;

export async function sendDiscord(webhookUrl: string, message: string): Promise<void> {
  // Discord has a 2000 character limit
  const content = message.length > MAX_CONTENT_LENGTH
    ? message.slice(0, MAX_CONTENT_LENGTH - 3) + "..."
    : message;

  const res = await fetch(webhookUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ content }),
  });

  if (!res.ok) {
    throw new Error(`Discord webhook failed: ${res.status} ${await res.text()}`);
  }
}
