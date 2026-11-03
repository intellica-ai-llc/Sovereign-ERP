# Run the data collector and see the claim card from the dashboard (free test setup)

One binary, `vc-server`, now serves the console AND runs the public market-data collector. The operator panel is at the
bottom of the console page ("Trading data and claim card"). It reads public data only; nothing in it can place an order.

## Start (GitHub Codespace, free)
    source "$HOME/.cargo/env"
    cargo build --release -p vc-server
    export VC_ADMIN_TOKEN="$(head -c 24 /dev/urandom | base64 | tr -d '/+=' )xyz1234"   # >= 24 chars; keep it
    echo "$VC_ADMIN_TOKEN"
    VC_DATA=$HOME/vcdata VC_ADDR=0.0.0.0:8080 ./target/release/vc-server
Open the Ports tab, make port 8080 public (or just click the forwarded address), open the page, then in the
"Trading data and claim card" panel paste the admin token and press Connect.

## Use
- Start: events to follow (default 20), seconds between rounds (default 300; minimum 30), rounds (0 = until stopped).
- Stop: ends the session at the next request. Each Start creates a new set of files under `$VC_DATA/trading/`
  (events, books, trades), each recording hash-chained. The panel shows the chain heads to commit to the evidence log.
- Run arbitrage scan: replays the newest books recording through the rebalancing scan (fee 4%, haircut 1%, min edge 2%).
- The card lists every check with PROVEN or the reason it is not yet. Today nothing can be proven: there is no paper ledger.

## Limits (honest)
- A Codespace stops after ~30 idle minutes, which also stops the collector; Start again when you return. For an
  unattended run use an always-on host (see HOSTING_FREE_TEST.md) with `VC_COLLECT=1`, which starts the collector at boot.
- Each session stops itself at 500 MB written. Defaults of 20 events every 5 minutes are deliberately light.
- If the first Start shows "discover failed", the error text on the panel is the validation result for Polymarket's
  API format or network access; send it back.
- The data tool works from a Codespace; from some countries or networks the API may be blocked. That is a network matter,
  separate from any trading-eligibility question (still open for Trinidad and Tobago).
