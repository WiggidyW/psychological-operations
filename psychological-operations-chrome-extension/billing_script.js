// Best-effort credentials extractor for console.x.com /
// developer.x.com. Listens for the popup's `extract_credentials`
// message and returns whatever fields it can find on the
// currently-displayed page DOM.
//
// Strategy: walk the page text and find labels like "Client ID",
// "API Key", etc. — for each match, take the closest sibling /
// adjacent value-shaped string (a `<code>`, an `<input value=…>`,
// or a `<span>` with a token-shaped run). Selectors here will need
// iteration the first time someone runs this against the live
// console.x.com DOM; treat as v1 best-effort. Missing fields come
// back as `null` and the host-side `merge` preserves any
// previously-captured value.

(function () {
  if (window.__psyopBillingScriptLoaded) return;
  window.__psyopBillingScriptLoaded = true;

  // Fields we look for on the page, in order of preference for a
  // given canonical name.
  const FIELDS = {
    client_id:      ["Client ID", "OAuth 2.0 Client ID"],
    client_secret:  ["Client Secret", "OAuth 2.0 Client Secret"],
    api_key:        ["API Key", "Consumer Key", "API Key (Consumer Key)"],
    api_key_secret: ["API Key Secret", "Consumer Secret", "API Key Secret (Consumer Secret)"],
    bearer_token:   ["Bearer Token"],
  };

  // Find the value associated with a given visible label. Walks
  // the DOM looking for text nodes equal to (or starting with) the
  // label, then climbs to the nearest container and pulls the
  // first descendant input/textarea/code element's value/text.
  function findValueForLabel(label) {
    const wantedLower = label.toLowerCase().trim();
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
      null,
    );
    let node;
    while ((node = walker.nextNode())) {
      const text = (node.nodeValue || "").trim().toLowerCase();
      if (text !== wantedLower && !text.startsWith(wantedLower)) continue;

      // Climb up a few levels to find a container that likely
      // wraps both the label and its value cell.
      let container = node.parentElement;
      for (let depth = 0; depth < 5 && container; depth++) {
        const found = pickValueFromContainer(container, label);
        if (found) return found;
        container = container.parentElement;
      }
    }
    return null;
  }

  // From a container element, try to extract a credential-shaped
  // string. Looks at <input value=…>, <textarea>, <code>, then
  // falls back to any descendant text that looks like a token
  // (long run of base64-ish chars).
  function pickValueFromContainer(container, label) {
    const inputs = container.querySelectorAll("input, textarea");
    for (const el of inputs) {
      const v = (el.value || "").trim();
      if (v && v.toLowerCase() !== label.toLowerCase()) return v;
    }
    const codes = container.querySelectorAll("code, pre");
    for (const el of codes) {
      const v = (el.textContent || "").trim();
      if (v && looksLikeToken(v)) return v;
    }
    // Fallback: any descendant span with a token-shaped run.
    const spans = container.querySelectorAll("span, div");
    for (const el of spans) {
      const v = (el.textContent || "").trim();
      if (v && v.length >= 20 && looksLikeToken(v)) return v;
    }
    return null;
  }

  function looksLikeToken(s) {
    // Tokens X dishes out are typically url-safe base64ish:
    // alphanumeric + - _ + maybe % + : (for bearer tokens).
    return /^[A-Za-z0-9_\-:%]{20,}$/.test(s);
  }

  function extract() {
    const out = {};
    for (const [key, labels] of Object.entries(FIELDS)) {
      let value = null;
      for (const label of labels) {
        value = findValueForLabel(label);
        if (value) break;
      }
      out[key] = value;
    }
    return out;
  }

  chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    if (!msg || typeof msg.kind !== "string") return false;
    if (msg.kind === "extract_credentials") {
      sendResponse({ credentials: extract() });
      return false;
    }
    return false;
  });
})();
