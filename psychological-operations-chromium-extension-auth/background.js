// Service worker. Owns a single chrome.runtime.connectNative port to
// the psychological-operations native host. Lazy-opens it; reopens
// on disconnect. Only relays the x_app credential save.

const HOST_NAME = "com.objectiveai.psychological_operations";

let port = null;
// Pending reply handlers, FIFO. Native messaging doesn't carry
// request IDs, so the protocol relies on one-in-flight-at-a-time
// discipline (the popup honors this — only one save at a time).
const pending = [];

function openPort() {
  if (port) return port;
  port = chrome.runtime.connectNative(HOST_NAME);
  port.onMessage.addListener((msg) => {
    const next = pending.shift();
    if (next) next.resolve(msg);
  });
  port.onDisconnect.addListener(() => {
    const err = chrome.runtime.lastError;
    while (pending.length) {
      const next = pending.shift();
      next.reject(new Error(err ? err.message : "native host disconnected"));
    }
    port = null;
  });
  return port;
}

function send(msg) {
  return new Promise((resolve, reject) => {
    let p;
    try { p = openPort(); }
    catch (e) { reject(e); return; }
    pending.push({ resolve, reject });
    try { p.postMessage(msg); }
    catch (e) {
      // remove the just-pushed handler since postMessage failed
      pending.pop();
      reject(e);
    }
  });
}

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (!msg || typeof msg.kind !== "string") return false;

  if (msg.kind === "popup_x_app_save") {
    send({ kind: "x_app_save", credentials: msg.credentials })
      .then((reply) => sendResponse(reply))
      .catch((e) => sendResponse({ kind: "x_app_save_err", error: String(e.message || e) }));
    return true;
  }

  return false;
});
