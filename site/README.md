# Pilot portal (static front end)

`index.html` is one self-contained page: downloads (read from `releases.json`), verification commands, licence instructions, manual index, support and security. No third-party scripts, fonts or trackers; content from `releases.json` is inserted as text, and links are limited to relative paths or `https`.

## Before hosting
1. Edit the `CONFIG` block at the bottom of `index.html` (support email, hours, docs URL).
2. Fill the severity targets from your pilot agreement (the table cells with class `sla`).
3. The release pipeline writes `releases.json` (see `releases.example.json` for the shape). Until it exists the page says no release is published.
4. The page describes the 0.1.0 evaluation build only: no licence file, commands `sov version|profile|audit|demo|digest`. Update the Licence and Support sections when licence verification and diagnostics ship.

## Hosting rules
- **Put it behind authentication** (SSO, per-customer accounts, or at minimum a reverse proxy with auth) and serve binaries through short-lived signed URLs from private storage. This page cannot protect files by itself.
- Serve over HTTPS with `Cache-Control: no-store` for `releases.json`, and a Content-Security-Policy such as `default-src 'self'; style-src 'unsafe-inline'; script-src 'unsafe-inline'`, tightened further once the inline script moves to its own file.
- The page sets `noindex`; keep the host out of search engines and public sitemaps.
