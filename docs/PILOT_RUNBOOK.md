# Pilot runbook: VeriChain on entirely free resources (checked 2026-11-03; confirm limits on each provider's page)

Path: Oracle Cloud Always Free VM + DuckDNS hostname + Caddy (automatic HTTPS) + UptimeRobot + backups to Oracle Object Storage.
Honest cost note: Oracle asks for a payment card to verify identity; nothing is charged while you stay inside Always Free.
A paid domain is NOT needed (DuckDNS gives a free subdomain). Everything below is a pilot setup: no SLA.

1. DuckDNS: sign in at duckdns.org, create a subdomain (e.g. `verichain-pilot`), note the token. Host = `verichain-pilot.duckdns.org`.
2. Oracle Cloud: create an account; choose the home region carefully (it cannot be changed; free ARM capacity varies).
3. Create the VM: Ubuntu 24.04, shape VM.Standard.A1.Flex with 2 OCPU / 12 GB (Always Free total), boot volume 50 GB, public IP.
   If "out of host capacity", retry later or in another availability domain. Fallback: VM.Standard.E2.1.Micro (1 GB: too small to compile; build elsewhere and copy the binary).
4. Network: in the VCN security list allow inbound TCP 80 and 443. On the VM also open the host firewall:
   `sudo iptables -I INPUT -p tcp --dport 80 -j ACCEPT; sudo iptables -I INPUT -p tcp --dport 443 -j ACCEPT; sudo netfilter-persistent save`
5. Point DuckDNS at the VM's public IP (duckdns.org page, "current ip"). Optional cron on the VM to keep it updated.
6. Install tools: `sudo apt update && sudo apt install -y caddy build-essential pkg-config git curl rclone age`; install Rust with rustup.
7. Build: `git clone https://github.com/intellica-ai-llc/verichain && cd verichain && cargo build --release -p vc-server -p vc-verify`
   (ARM build on the VM). `sudo install target/release/vc-server /usr/local/bin/`
8. Service user and data dir: `sudo useradd -r -s /usr/sbin/nologin vcsrv; sudo mkdir -p /var/lib/vc-server; sudo chown vcsrv: /var/lib/vc-server`
9. Config: `sudo cp deploy/vc-server.env.example /etc/vc-server.env; sudo chmod 600 /etc/vc-server.env`; edit it: a long random
   VC_ADMIN_TOKEN (`openssl rand -base64 32`), VC_HOST=your DuckDNS host. Save the token in a password manager.
10. Service: `sudo cp deploy/vc-server.service /etc/systemd/system/ && sudo systemctl enable --now vc-server`; check `journalctl -u vc-server -n 20`.
11. HTTPS: `/etc/caddy/Caddyfile`:
        verichain-pilot.duckdns.org {
            reverse_proxy 127.0.0.1:8080 {
                header_up X-Real-IP {remote_host}
            }
        }
    then `sudo systemctl reload caddy`. Caddy gets and renews a Let's Encrypt certificate by itself.
12. Open `https://<host>/`, use the operator panel with the admin token. Create the first tenant:
        curl -s -X POST https://<host>/admin/v1/tenants -H "Authorization: Bearer $VC_ADMIN_TOKEN" -d '{"id":"pilot1","name":"Pilot 1"}'
    The tenant key is shown ONCE; hand it to the pilot user.
13. Backups: create an Oracle Object Storage bucket (private), create a customer secret key, `rclone config` an S3-compatible remote,
    then cron `17 3 * * * VC_DATA=/var/lib/vc-server BACKUP_REMOTE=oci:vc-backups AGE_RECIPIENT=<age public key> /opt/verichain/backup.sh`.
    Also download one encrypted backup to your own computer monthly (a second provider). RESTORE DRILL: stop the service, untar a backup into
    a scratch dir, start a second instance on another port against it, confirm `/v1/checkpoint` matches. Do this once before real users.
14. Monitoring: UptimeRobot (free) HTTP monitors on `https://<host>/readyz` (fails if the data directory is unwritable or, with VC_COLLECT=1, the collector is down) and `/healthz`; e-mail alert. Watch the collector in the operator panel.
15. Oracle idle-reclaim risk: Oracle may reclaim Always Free instances that are idle for 7 days (CPU 95th percentile < 20%, network < 20%,
    and memory < 20% on A1). A quiet pilot server can look idle. Check Oracle's current policy; the usual mitigation is upgrading the
    account to Pay As You Go (still free inside the limits). Verify that on Oracle's page before relying on it.
16. Dependency audit (free, in a Codespace): `cargo install cargo-audit && cargo audit`.
17. Pilot paperwork (needs a lawyer, not code): pilot terms (no SLA, data handling, deletion), privacy notice, security contact.

## Independent timestamps (RFC 3161), after the pilot is up
- A tenant calls `POST /v1/anchor` (Bearer tenant key); the latest checkpoint is hashed and sent to FreeTSA (`VC_TSA_URL` to change). The reply is stored under `logs/<tenant>/anchors/` and listed by `GET /v1/anchors`.
- Verify any stored anchor with standard tools: `curl -O https://freetsa.org/files/cacert.pem -O https://freetsa.org/files/tsa.crt`, decode `reply_b64` to `reply.tsr`, then
  `openssl ts -verify -in reply.tsr -digest <digest_sha256> -CAfile cacert.pem -untrusted tsa.crt`.
- FreeTSA states no rate limit or uptime guarantee: schedule anchoring at most hourly, and treat it as a bonus, not a dependency.
- Admin audit log: `GET /admin/v1/audit` (admin token) shows operator actions and whether the chain is intact. Copy its head somewhere else after big changes.
