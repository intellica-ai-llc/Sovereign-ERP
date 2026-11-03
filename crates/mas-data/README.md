# mas-data: record real Polymarket books and scan them (read-only, no orders)

Run these in a Codespace (the cloud container used for development has no route to Polymarket):

    cargo build --release -p mas-data
    ./target/release/mas-data discover --out events.json --limit 100
    ./target/release/mas-data record --events events.json --out rec1.jsonl --rounds 20 --interval 30
    ./target/release/mas-data verify rec1.jsonl       # prints the head hash: commit it to the evidence log
    ./target/release/mas-data scan rec1.jsonl events.json --fee-rate 0.04 --haircut 0.01 --min-edge 0.02

Notes
- The recording is hash-chained; edits, deletions and reordering are detected. Commit its head hash to your VeriChain log
  when you close a recording, BEFORE analysing it, so nobody (including us) can later cherry-pick the data.
- Only negRisk events are recorded and scanned. NegRisk events can hold hidden placeholder outcomes, so a "gap" is a
  CANDIDATE for human review, not an order.
- Parsers were written from the public docs and a third-party client's type definitions, not the live API. If `discover`
  or `record` reports a field error, send me the message: the parsers refuse unknown shapes on purpose.
- Be polite: the recorder waits 120 ms between requests; do not lower it.
