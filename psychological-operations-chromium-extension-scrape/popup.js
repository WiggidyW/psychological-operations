const identityEl = document.getElementById("identity");
const captureBtn = document.getElementById("capture");
const statusEl   = document.getElementById("status");

let activeTabId = null;
let countTimer  = null;

function setStatus(text, cls) {
  statusEl.textContent = text;
  statusEl.className = cls || "";
}

async function activeTab() {
  if (activeTabId != null) return activeTabId;
  const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
  activeTabId = tabs[0] ? tabs[0].id : null;
  return activeTabId;
}

async function refreshCount() {
  const id = await activeTab();
  if (id == null) return;
  try {
    const reply = await chrome.tabs.sendMessage(id, { kind: "count" });
    const n = (reply && reply.count) || 0;
    captureBtn.textContent = `Capture (${n} tweet${n === 1 ? "" : "s"})`;
    captureBtn.disabled = n === 0;
  } catch (_) {
    captureBtn.textContent = "Capture (not an X page)";
    captureBtn.disabled = true;
  }
}

async function loadIdentity() {
  let reply;
  try {
    reply = await chrome.runtime.sendMessage({ kind: "popup_get_identity" });
  } catch (e) {
    identityEl.textContent = `identity error: ${e.message || e}`;
    identityEl.classList.add("error");
    return;
  }
  if (reply && reply.ok) {
    const id = reply.identity;
    identityEl.textContent = `psyop: ${id.psyop} @ ${id.commit.slice(0, 8)}`;
    identityEl.classList.remove("error");
  } else {
    // The scrape extension only makes sense in psyop context. If
    // PSYOP_NAME isn't set the launcher pointed us at the wrong
    // session — surface the error rather than silently disabling.
    identityEl.textContent = `no psyop identity: ${(reply && reply.error) || "unknown"}`;
    identityEl.classList.add("error");
    captureBtn.disabled = true;
    captureBtn.textContent = "Capture (no psyop identity)";
  }
}

captureBtn.addEventListener("click", async () => {
  captureBtn.disabled = true;
  setStatus("extracting…");
  try {
    const id = await activeTab();
    const extractReply = await chrome.tabs.sendMessage(id, { kind: "extract" });
    const tweets = (extractReply && extractReply.tweets) || [];
    if (tweets.length === 0) {
      setStatus("nothing to capture", "error");
      captureBtn.disabled = false;
      return;
    }
    setStatus(`sending ${tweets.length}…`);
    const reply = await chrome.runtime.sendMessage({ kind: "popup_ingest", tweets });
    if (reply.kind === "ingest_ok") {
      setStatus(`inserted ${reply.inserted}, skipped ${reply.skipped}`, "ok");
    } else {
      setStatus(`error: ${reply.error || "?"}`, "error");
    }
  } catch (e) {
    setStatus(`error: ${e.message || e}`, "error");
  } finally {
    captureBtn.disabled = false;
    refreshCount();
  }
});

window.addEventListener("unload", () => {
  if (countTimer) clearInterval(countTimer);
});

(async () => {
  await loadIdentity();
  refreshCount();
  countTimer = setInterval(refreshCount, 500);
})();
