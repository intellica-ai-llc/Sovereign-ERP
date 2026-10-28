# Getting started in Codespaces

1. Open a Codespace on `intellica-ai-llc/Sovereign-ERP` (github.dev in the browser cannot run `cargo`; use a Codespace).
2. Drag `sovereign-erp-code-v2.zip` into the Explorer panel.
3. In the terminal:
   ```sh
   cd /workspaces/Sovereign-ERP
   unzip -o sovereign-erp-code-v2.zip && rm sovereign-erp-code-v2.zip
   cd code && cargo test --workspace
   ```
   If `cargo` is not found, run "Codespaces: Rebuild Container" from the Command Palette.
4. Commit and push:
   ```sh
   cd /workspaces/Sovereign-ERP
   git add -A && git commit -m "Add Rust workspace: period controller and ledger port" && git push
   ```
