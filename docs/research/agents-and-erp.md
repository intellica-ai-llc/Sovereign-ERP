# LLM agents and ERP: landscape, threats, and our design

Status: 2026-11. Evidence labels: *vendor*, *independent*, *inferred*. Nothing here was tested against a competitor; **the gateway described under "Our design" is not built.**

## How the incumbents connect agents to ERP
SAP, Oracle and Microsoft all ship agentic features: SAP's Joule agents (with MCP connections discussed in SAP community material), Oracle Fusion agentic applications, and Dynamics 365 ERP opened to agents through MCP servers under existing role-based security and approval steps. *(vendor)* One trade report says SAP restricts external agents while ServiceNow and Salesforce are more open. *(vendor/trade press)* Most of this is cloud-delivered. A vendor-published analysis cites Gartner as expecting over 40 percent of agentic AI projects to be cancelled by 2027. *(vendor citing analyst; unverified)*

Pattern: MCP as the plumbing; the agent inherits a user's role; risky steps wait for human approval; actions are logged. *(vendor)*

## The threat that pattern does not answer
An agent acting with a user's rights is a **confused deputy** (Hardy, 1988): text it reads (an invoice note, a work-order comment) can steer it to use the user's full authority. OWASP lists prompt injection and excessive agency as separate risks; the second turns the first into a breach. *(independent)* Research from Google DeepMind and ETH Zurich (CaMeL) answers with a design: a privileged planner sees only the trusted request and writes the control flow; a quarantined model parses untrusted data but has no tools; an interpreter tracks where each value came from and enforces policy before every tool call. Reported result: provable security on about 77 percent of benchmark tasks against 84 percent undefended. *(independent, arXiv 2503.18813 and summaries)*

## Our design (not built)
The generic research secures an interpreter around arbitrary tools. Our tools are **verified state machines**, which adds what a policy layer alone cannot:

1. **Typed tools, deny by default.** Each tool declares argument types and a risk class; an agent session holds explicit capabilities (tool, amount ceiling, call budget), narrower than any user's rights.
2. **Taint tracking.** Values read from the ERP or from documents are untrusted. A medium- or high-risk call whose effect-controlling arguments (a payment destination, a recipient) derive from untrusted text cannot auto-commit.
3. **Kernel dry run.** Every call is previewed against the real period-close, ledger and lifecycle rules; the kernel refuses anything that breaks an invariant, however the request was phrased.
4. **Approval bound to the intent.** An approval covers one exact intent (a hash of tool, arguments and requester) and must come from a different person.
5. **Replayable, tamper-evident audit.** Each decision is appended to a hash chain; replaying the committed intents from a fresh state must reproduce the live state exactly.
6. **MCP-compatible manifest.** Tool schemas export as MCP `tools/list` entries so any model or client can use the gateway.

Properties to verify before claiming any of this: no state change without a capability; no untrusted-controlled medium or high risk effect without approval; approvals not reusable on changed arguments; every commit has an audit record; replay equals live state; any altered audit byte is detected. The hash chain needs SHA-256, which now exists (`sov-evidence`).

## Why this could be a moat, and how it could fail
Competitors can add approvals and logs; fewer can show that a kernel, not a prompt, protects the books, or let an auditor replay the actions. It fails if buyers value breadth of agent features over provable safety, or if a platform vendor adopts the same capability-and-taint discipline first. *(inferred)*

## Next
Build the gateway with the properties above as tests; add an MCP adapter; evaluate against prompt-injection benchmarks with a real model in the loop (not done).
