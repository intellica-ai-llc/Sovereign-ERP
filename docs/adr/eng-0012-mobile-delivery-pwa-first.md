# eng-0012: Mobile delivery: installable web app first, native shells later

- Status: proposed (resolves arc42 ADR 029 if the spike below succeeds)
- Date: 2026-11-02

## Context
Pilots need technicians to use the product on their own phones and rugged tablets, offline, without waiting on app-store review or a customer's device-management policy. Competitors compete on offline work orders, QR and barcode scanning, photo and voice capture, push alerts, automatic sync and on-device AI.

## Decision (to be confirmed by a spike)
1. **First delivery: an installable web app (PWA)** with the sync core compiled to WebAssembly and local storage in the browser. One codebase, no store review, works on any current phone or tablet; QR scanning, camera, voice capture and offline storage are available to web apps.
2. **Later: native shells** (Swift and Kotlin) over the same Rust core through generated bindings, when a customer needs background sync, push on locked devices, NFC or rugged-scanner integration beyond what browsers allow.
3. The core stays transport- and UI-neutral (`sov-sync` has no I/O), so either shell is replaceable.

## Spike (must pass before this is accepted)
Compile `sov-sync` to WebAssembly, run the convergence simulation in a browser worker, measure bundle size and storage limits on a mid-range Android phone and an iPhone, and test a full offline shift (airplane mode) including a forced browser storage eviction. Record what fails.

## Consequences
Fastest path to a pilot and no dependency on app stores. Risks: browser storage can be evicted and background sync is limited on iOS, so the app must treat the server as the safety copy and sync eagerly while online. Not yet tested here: the WebAssembly build target is not installed in the current build environment.
