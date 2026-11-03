# vcd: try VeriChain in two minutes
    cargo build --release -p vcd -p vc-verify
    ./target/release/vcd demo  ~/vc-demo      # makes a signed evidence pack from two demo agents
    ./target/release/vcd serve ~/vc-demo      # open http://127.0.0.1:8787 (Codespaces: open the forwarded port)
    ./target/release/vcd verify ~/vc-demo     # command line check, exit 0 = all checks pass
    ./target/release/vcv verify ~/vc-demo/pack.json --trust ~/vc-demo/trust.json --policy ~/vc-demo/policy.json --require-seals
In the dashboard, press the tamper buttons: editing a past statement or hiding the latest activity must turn checks red.
