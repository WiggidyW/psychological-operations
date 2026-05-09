# psychological-operations-chromium-extension-auth

MV3 extension that captures the master X-App credentials from
`developer.x.com` / `console.x.com` and ships them to the native
host. The host writes them to
`~/.psychological-operations/x_app.json`, where the per-psyop
OAuth flow reads them.

This is one of two extensions bundled into
`psychological-operations`. The other,
[`-extension-scrape`](../psychological-operations-chromium-extension-scrape),
captures For-You tweet IDs on `x.com`. They have **separate
extension IDs** — derived from `extension-key-auth.pem` and
`extension-key-scrape.pem` respectively — and are never loaded
into the same Chromium profile.

## Files

- `manifest.json` — MV3 manifest. `host_permissions` covers
  `developer.x.com` + `console.x.com`. **No `content_scripts`**
  (no DOM walker; the popup form is the only surface).
- `popup.html` / `popup.js` / `popup.css` — the 5-field credential
  form (Client ID, Client Secret, API Key, API Key Secret, Bearer
  Token).
- `background.js` — service worker. Single
  `chrome.runtime.connectNative` port relays the
  `popup_x_app_save` message to the host.

## Launch surface

The CLI's `psychological-operations x_app setup` subcommand:

1. Materializes the bundled Chromium + this extension into
   `~/.psychological-operations/chromium/<hash>/`.
2. Registers the native-messaging host.
3. Spawns Chromium with `--user-data-dir=<auth profile>` +
   `--load-extension=<auth dir>` +
   `--allowlisted-extension-id=<auth id>`, landing on
   `https://developer.x.com/en/portal/projects-and-apps`.

The operator signs in, creates a Project + App on developer.x.com,
copies the credentials from the keys-and-tokens page into this
extension's popup form, hits Save, and the host writes
`x_app.json`.
