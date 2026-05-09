const xAppForm   = document.getElementById("x_app_form");
const xAppSaveBtn = document.getElementById("bf_save");
const statusEl   = document.getElementById("status");

const X_APP_FIELDS = [
  ["client_id",     "bf_client_id"],
  ["client_secret", "bf_client_secret"],
  ["bearer_token",  "bf_bearer_token"],
];

function setStatus(text, cls) {
  statusEl.textContent = text;
  statusEl.className = cls || "";
}

xAppForm.addEventListener("submit", async (ev) => {
  ev.preventDefault();
  xAppSaveBtn.disabled = true;

  const credentials = {};
  let nonEmpty = 0;
  for (const [key, inputId] of X_APP_FIELDS) {
    const v = document.getElementById(inputId).value.trim();
    if (v.length > 0) {
      credentials[key] = v;
      nonEmpty++;
    } else {
      credentials[key] = null;
    }
  }
  if (nonEmpty === 0) {
    setStatus("enter at least one field", "error");
    xAppSaveBtn.disabled = false;
    return;
  }

  setStatus(`saving ${nonEmpty} field${nonEmpty === 1 ? "" : "s"}…`);
  try {
    const reply = await sendToBackground({ kind: "popup_x_app_save", credentials });
    if (reply && reply.kind === "x_app_save_ok") {
      setStatus(`saved ${nonEmpty} field${nonEmpty === 1 ? "" : "s"} to x_app.json`, "ok");
      // Clear inputs after a successful save so secrets don't linger
      // visible in the popup if the operator re-opens it.
      for (const [_key, inputId] of X_APP_FIELDS) {
        document.getElementById(inputId).value = "";
      }
    } else {
      setStatus(`error: ${(reply && reply.error) || "?"}`, "error");
    }
  } catch (e) {
    setStatus(`error: ${e.message || e}`, "error");
  } finally {
    xAppSaveBtn.disabled = false;
  }
});

// chrome.runtime.sendMessage is unreliable on first call against an
// inactive MV3 service worker ("Receiving end does not exist" race).
// Use chrome.runtime.connect — opening a port reliably wakes the SW
// and keeps it alive until we explicitly disconnect.
function sendToBackground(msg) {
  return new Promise((resolve, reject) => {
    let port;
    try { port = chrome.runtime.connect({ name: "popup" }); }
    catch (e) { reject(e); return; }
    let settled = false;
    port.onMessage.addListener((reply) => {
      if (settled) return;
      settled = true;
      port.disconnect();
      resolve(reply);
    });
    port.onDisconnect.addListener(() => {
      if (settled) return;
      settled = true;
      const err = chrome.runtime.lastError;
      reject(new Error(err ? err.message : "background port disconnected"));
    });
    try { port.postMessage(msg); }
    catch (e) {
      if (!settled) { settled = true; reject(e); }
    }
  });
}
