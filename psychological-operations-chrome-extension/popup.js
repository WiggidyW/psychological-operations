const identityEl     = document.getElementById("identity");
const captureBtn     = document.getElementById("capture");
const credentialsBtn = document.getElementById("save_credentials");
const statusEl       = document.getElementById("status");

let activeTabId = null;
let activeUrl   = null;
let countTimer  = null;

function setStatus(text, cls) {
  statusEl.textContent = text;
  statusEl.className = cls || "";
}

async function activeTab() {
  if (activeTabId != null) return { id: activeTabId, url: activeUrl };
  const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
  if (tabs[0]) {
    activeTabId = tabs[0].id;
    activeUrl   = tabs[0].url || "";
  }
  return { id: activeTabId, url: activeUrl };
}

function isConsoleHost(url) {
  return /^https:\/\/(console|developer)\.x\.com\//.test(url || "");
}

function isXHost(url) {
  return /^https:\/\/(x|twitter)\.com\//.test(url || "");
}

async function applyTabContext() {
  const { url } = await activeTab();
  if (isConsoleHost(url)) {
    captureBtn.hidden = true;
    credentialsBtn.hidden = false;
  } else {
    captureBtn.hidden = false;
    credentialsBtn.hidden = true;
  }
}

async function refreshCount() {
  if (captureBtn.hidden) return;
  const { id, url } = await activeTab();
  if (id == null) return;
  if (!isXHost(url)) {
    captureBtn.textContent = "Capture (not an X page)";
    captureBtn.disabled = true;
    return;
  }
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
  try {
    const reply = await chrome.runtime.sendMessage({ kind: "popup_get_identity" });
    if (reply && reply.ok) {
      const id = reply.identity;
      identityEl.textContent = `psyop: ${id.psyop} @ ${id.commit.slice(0, 8)}`;
      identityEl.classList.remove("error");
    } else {
      // On the billing profile no PSYOP_NAME is set; the identity
      // request returns an error. Render a friendlier label.
      identityEl.textContent = "billing setup";
      identityEl.classList.remove("error");
    }
  } catch (e) {
    identityEl.textContent = `identity error: ${e.message || e}`;
    identityEl.classList.add("error");
  }
}

captureBtn.addEventListener("click", async () => {
  captureBtn.disabled = true;
  setStatus("extracting…");
  try {
    const { id } = await activeTab();
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

credentialsBtn.addEventListener("click", async () => {
  credentialsBtn.disabled = true;
  setStatus("scraping…");
  try {
    const { id } = await activeTab();
    const extractReply = await chrome.tabs.sendMessage(id, { kind: "extract_credentials" });
    const credentials = (extractReply && extractReply.credentials) || {};
    const found = Object.entries(credentials).filter(([_, v]) => v).length;
    if (found === 0) {
      setStatus("no credentials visible on this page", "error");
      credentialsBtn.disabled = false;
      return;
    }
    setStatus(`saving ${found} field${found === 1 ? "" : "s"}…`);
    const reply = await chrome.runtime.sendMessage({ kind: "popup_billing_save", credentials });
    if (reply.kind === "billing_save_ok") {
      setStatus(`saved ${found} field${found === 1 ? "" : "s"} to billing.json`, "ok");
    } else {
      setStatus(`error: ${reply.error || "?"}`, "error");
    }
  } catch (e) {
    setStatus(`error: ${e.message || e}`, "error");
  } finally {
    credentialsBtn.disabled = false;
  }
});

window.addEventListener("unload", () => {
  if (countTimer) clearInterval(countTimer);
});

loadIdentity();
applyTabContext().then(refreshCount);
countTimer = setInterval(refreshCount, 500);
