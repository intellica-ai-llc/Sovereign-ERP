# Security and privacy statement (evaluation build 0.1.0)

Facts about the `sov` tool, each checkable:

- **Local only.** It reads files you point it at and writes to standard output. It contains no network code and opens no connections. To check, run it in a container or VM with networking disabled; behaviour is identical.
- **No telemetry.** Nothing is collected, logged elsewhere or sent.
- **Read-only.** It never modifies its input; a test hashes every input file before and after a run. The only file it ever writes is `sov demo --export DIR`, which creates new files of synthetic data in a directory you name.
- **Deterministic.** Same input, same output, same digest. Reports end with the SHA-256 of the text above, so a report you send us can be re-verified.
- **What reports contain.** The profile report (text and JSON) holds table and column names, counts, relationship structure and dictionary relationship names and reasons. It holds no row values. The `audit` report **does** print the key values of drifting balances.
- **Dependencies.** No third-party code at runtime. A CycloneDX bill of materials ships with each build.
- **Build integrity.** Releases are built in CI from a tag and packaged reproducibly with SHA-256 checksums. Signing and provenance are written but not yet exercised (see `STATUS.md`).

Limits you should know:

- **Hostile input.** The loader rejects malformed files with an error, and tests cover garbage inputs, but the tool has **not been fuzzed or independently reviewed**. Do not run it on untrusted files in a sensitive environment.
- **Memory.** Tables are read fully into memory up to the row cap; very wide tables use more.
- **Not a security control.** It reports on data; it does not protect it.

Report a suspected vulnerability privately (see `SECURITY.md`). A named contact and disclosure timeline will be published before the first customer deployment.
