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
    const reply = await chrome.runtime.sendMessage({ kind: "popup_x_app_save", credentials });
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
