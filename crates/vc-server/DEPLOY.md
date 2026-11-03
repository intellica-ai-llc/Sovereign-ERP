# Deploying vc-server (hosted VeriChain log)

## Run
    export VC_ADMIN_TOKEN="$(head -c 32 /dev/urandom | xxd -p -c 64)"   # keep this secret; needed to create tenants
    export VC_DATA=/var/lib/verichain   VC_HOST=log.yourdomain.com   VC_ADDR=127.0.0.1:8080
    ./target/release/vc-server
Put TLS in front (Caddy: `log.yourdomain.com { reverse_proxy 127.0.0.1:8080 }`). Back up `VC_DATA` (tenants.json, operator.key, logs/).

## Create a tenant, register an agent, send a statement
    curl -s -X POST https://HOST/admin/v1/tenants -H "Authorization: Bearer $VC_ADMIN_TOKEN" -d '{"id":"acme","name":"Acme"}'
    # -> api_key (shown once) and operator_public_key (customers pin this)
    curl -s -X POST https://HOST/v1/agents -H "Authorization: Bearer $KEY" -d '{"id":"agent-1","public_key":"<ed25519 hex>"}'
    curl -s -X POST https://HOST/v1/statements -H "Authorization: Bearer $KEY" --data-binary @statement.cose
    curl -s https://HOST/v1/checkpoint -H "Authorization: Bearer $KEY"
Console: open `https://HOST/` and paste the tenant API key.

## Not in this slice yet (see ROADMAP)
witness cosigning service, evidence-pack export endpoint, rate limits and per-plan quotas from signed licenses, user accounts/SSO, metrics, WASM verify-in-browser.

## Run an independent witness (separate machine, separate key)
    cargo build --release -p vc-witness
    ./target/release/vcw keygen witness.key            # prints the witness PUBLIC key
    # log owner registers it:  POST /v1/witnesses {"name":"w1","public_key":"<hex>"}
    ./target/release/vcw run --log https://HOST --origin HOST/TENANT --operator-key <operator key from tenant creation> \
        --key-file witness.key --name w1 --state-dir ./w1-state --token <tenant API key> --every 60
A witness refuses rollbacks, refuses forks (and freezes that log with signed proof), and saves its state before it cosigns.
Packs are anchored at the newest checkpoint cosigned by at least `min_witnesses` registered witnesses.
Follow-up: scoped witness tokens (today the witness uses the tenant key; it should only be allowed to read checkpoints and post cosignatures).

## Witnesses (independent cosigning)
Run `vc-witnessd` on a DIFFERENT host/operator than the log (that independence is the point):
    WITNESS_NAME=w1 WITNESS_DIR=/var/lib/vcw WITNESS_ADDR=127.0.0.1:8102 ./target/release/vc-witnessd   # prints its public key
    echo "log.example.com/acme operator <operator public key from vc-server startup> " >> /var/lib/vcw/logs.txt   # origin, key name, key hex (one line, space separated)
Register it with the log (once): `POST /v1/witnesses {"name":"w1","public_key":"<hex>"}` with the tenant key.
Then run `scripts/witness-loop.sh` every minute (needs curl and jq): LOG_URL, LOG_KEY, WITNESS_URL, STATE=./last-size.
`GET /v1/pack` then serves packs anchored at the newest checkpoint cosigned by enough witnesses; `vcv verify` passes them.

## Phone approvals (passkeys)
Open `https://HOST/approve` on a phone, paste the tenant key, tap "Enrol this device's passkey", then approve or deny pending
gated actions. Set VC_ORIGIN only for local tests (WebAuthn needs https, or http://localhost). RP id = the host name.
An agent marks an intent with `ext.requires_approval: true` (and optionally `ext.summary`, `ext.action_class`); relying parties pin
`gated_classes` in their policy so an issuer cannot skip the gate.
