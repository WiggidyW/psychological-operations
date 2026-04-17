const MAX_MESSAGE_LENGTH = 4096;

export async function sendTelegram(botToken: string, chatId: string, message: string): Promise<void> {
  // Telegram has a 4096 character limit
  const text = message.length > MAX_MESSAGE_LENGTH
    ? message.slice(0, MAX_MESSAGE_LENGTH - 3) + "..."
    : message;

  const url = `https://api.telegram.org/bot${botToken}/sendMessage`;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ chat_id: chatId, text, parse_mode: "Markdown" }),
  });

  if (!res.ok) {
    throw new Error(`Telegram API failed: ${res.status} ${await res.text()}`);
  }
}
