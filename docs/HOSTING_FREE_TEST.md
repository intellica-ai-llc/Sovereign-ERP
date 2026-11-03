# Free ways to host and test vc-server (checked 2026-11-01; limits change, confirm on the provider's page)

1. Quickest test, no card: GitHub Codespaces. Run vc-server in the Codespace, open the Ports tab, set port 8080 to Public, share the URL. Data lives while the Codespace exists; it stops when idle. Fine for demos, not for customers.
2. Always-on test VM: Google Cloud e2-micro (always-free in us-west1/us-central1/us-east1; card required) or Oracle Cloud Always Free (card required; ARM allowance was cut to about 2 vCPU / 12 GB in 2026 and idle instances can be reclaimed). Either runs the Docker image or the bare binary behind Caddy for TLS.
3. Not suitable: Render free (sleeps after 15 minutes, ephemeral disk), Fly.io (trial only), Railway (30-day trial), Vercel/Netlify/Cloudflare Pages (static or serverless; the log needs a persistent disk).
4. For real customers: a paid small VPS (about US$4-6/month) with a backup of VC_DATA. Free tiers are for testing only.

Update 2026-11-03 (from provider pages read today): Oracle Always Free = up to 2 AMD micro VMs (1 GB each) or A1 ARM with 2 OCPU / 12 GB total,
200 GB block storage, 10 TB/month outbound, Object Storage 20 GB; idle instances may be reclaimed (7 days: CPU p95 < 20%, network < 20%,
memory < 20% on A1). Cloudflare R2 free = 10 GB, no egress fees, but a payment method is required and usage above free is billed.
Cloudflare Tunnel is free but needs a domain on Cloudflare DNS (domains are not free); DuckDNS + Caddy avoids that. UptimeRobot free:
50 monitors at 5-minute checks (third-party summary: commercial use allowed since June 2026; confirm). FreeTSA offers free RFC 3161
timestamps (no stated rate limit or SLA). Full pilot steps: docs/PILOT_RUNBOOK.md.
