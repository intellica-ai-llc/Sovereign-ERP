# MAS Addendum I: verified performance as the headline (2026-10-31)

Status: proposal for operator approval; amends MAS ARC42 v4 sections 4, 5.2 and 10.1 once approved.

## 1. Thesis
Nobody can be trusted to claim "record returns"; most such claims are unverifiable and many are false. What VeriChain can
produce that no competitor shows is a trading track record that is **pre-registered, hash-chained, reconciled to venue
data and verifiable offline by a stranger**. That is the news. Raw return is not the headline, and no return is promised.
Metrics we will aim to lead on, each one auditable:
1. Fraction of orders with a registered intent before signing (target 100%, gate Q1/Q5).
2. Reconciliation error between agent ledger and venue/chain data (target 0 breaks).
3. Deflated Sharpe at the true, publicly committed trial count (the registry head is published).
4. Realised versus assumed fill/markout slippage (published monthly).
5. Maximum drawdown versus the pre-declared limit (never breached because the signer enforces it).

## 2. Frontier evidence used (all `[S]`, unverified by us until replicated)
- A 2026 study of 822 resolved Polymarket questions found a frontier LLM forecaster earned +4.0% ROI against the market's
  -0.5%, rising to +9.1% when crowd agreement among 32 model configurations was 70-90%. Most of the gain came from the
  **price paid** (taking the NO side of overpriced YES outcomes; favourite-longshot bias), not from better accuracy;
  single-shot, no sizing, no market impact ([Lacuna summary](https://lacuna.tiptreesystems.com/work/beyond-accuracy-can-llm-forecasters-profit-on-prediction-markets/wrk_58f2322089a110fc5d773e7231e0dd88)).
  Consequence for FC-1: trade only the **moderate-agreement band**, and rank candidates by price-paid edge, not by
  forecast accuracy. Treat the study as a hypothesis to replicate in our simulator, not as an expectation.
- A belief-to-trade layer separate from the forecaster improves outcomes ([arXiv 2607.03015](https://arxiv.org/html/2607.03015v1)); already M04.
- Calibrated agents as market makers for long-tail markets ([Springer 2026](https://link.springer.com/article/10.1007/s42521-026-00233-z)); abstract only was reachable, so LP-1 variants from it need a full read before use.
- Prior Kalshi evidence (-9.64% makers, -31.46% takers on average) stands: selection, not being a maker, is the edge.

## 3. Changes to the spec
| # | Change | Where |
|---|---|---|
| I1 | Every trial, including abandoned ones, is pre-registered in the hash-chained registry (`mas-research::registry`) before it runs; the head hash is published as a VeriChain statement at least weekly | 4, 10.1 S0 |
| I2 | S0 exit uses `Registry::deflated` (trial count = all registered trials of the family; variance over recorded Sharpes), not a hand-entered count | 10.1 |
| I3 | FC-1 restricted to the 70-90% agreement band and ranked by price-paid edge; sports still excluded; replicate the +9.1% claim in the simulator before any paper trading | 5.2 |
| I4 | Public track-record page is generated only from `mas-pack` output; no number appears that is not in a verifiable pack; no forward-looking return claims | 5.1, M20 |
| I5 | Negative results are published too (a failed gate is a pre-registered outcome, not an embarrassment) | 11 |

## 3b. Completeness-proved forecast skill (I6, new)
Cherry-picking is the main way track records lie. VeriChain already proves completeness by construction (every intent
is registered before its effect), so we can score forecasts **only against a market universe committed before outcomes**
and refuse any score when the agent did not forecast (almost) all of it. `mas-research::scoring` implements this:
universe commitment (order-independent hash), coverage gate, Brier skill versus the market price, a paired one-sided
p-value, and calibration error. Headline form: "forecast every market in a committed universe; beat the market's own
price on Brier score with p < 0.01 over N markets; the full set is verifiable offline." This is independent of fees and
of position sizing, so it can be proven before any money is risked. Whether this combination is unique is untested:
we have not surveyed all competitors, so we do not claim uniqueness until we have.

## 3c. Competitor round 1 (2026-10-31; two drafts read, marketing claims not verified)
| Competitor | What it does | Gap we can own |
|---|---|---|
| VeritasChain VCP v1.1 ([IETF draft-kamimura-scitt-vcp-02](https://datatracker.ietf.org/doc/html/draft-kamimura-scitt-vcp-02)) | Signed (Ed25519, optional Dilithium), sequenced, Merkle-logged trading events; SCITT-style receipts; Silver/Gold/Platinum tiers; claims regulator submissions in many jurisdictions (press releases, unverified) | Its own text says completeness has "no absolute guarantees"; per-actor chains optional at Silver/Gold; anchoring optional at protocol level; no separate intent-before-effect record; no performance or track-record scoring |
| Noa AI-agent action receipts ([draft-noa-scitt-ai-agent-receipt-01](https://www.ietf.org/archive/id/draft-noa-scitt-ai-agent-receipt-01.html)) | Per-action signed JSON receipt, hash-chained, COSE-wrapped, can register with SCITT | Tail truncation undetectable without a checkpoint; no equivocation detection; timestamps are signer-chosen; keyed commitments not checkable by an offline third party |
| Forecasting platforms (Metaculus etc.) | Track records inside a platform the user must trust; a Metaculus Tezos timestamping validator exists | Not agent-signed, not completeness-proved, not tied to trading evidence |
Positioning: VCP is the closest and is already in IETF circles, so we should interoperate (export our packs as SCITT receipts, M20)
and compete on what its text concedes it lacks: absolute completeness, mandatory anchoring, intent-before-effect, and scored skill.

### New ideas from this round
- I7 Epoch seal (BUILT: `Emitter::seal`, `vcv verify --require-seals`, `Pins::require_seals`; tests prove a truncated pack passes without it and fails only on `epoch_seal` with it): each agent chain closes every epoch with a signed seal naming its final seq and statement count; a verifier requires a
  seal covering the claimed window, which closes the tail-truncation hole both drafts admit.
- I8 Pre-resolution proof (built): a forecast counts only if an EXTERNAL anchor (RFC 3161 or OpenTimestamps) proves it existed at least
  `min_lead_secs` before market close; signer-chosen timestamps are never accepted (`scoring::score` refuses otherwise).
- I9 Openable commitments: tenant-keyed HMAC commitments cannot be checked by an offline third party; add an opt-in selective
  disclosure (reveal key or salt per commitment to a named auditor) so audits remain possible without publishing parameters.
- I10 SCITT/VCP interop: emit our receipts in SCITT form and ingest VCP events, so adoption does not require abandoning an existing audit trail.

## 3d. Competitor round 2: AI trading agents (2026-10-31)
[PolyBench](https://arxiv.org/html/2604.14199v1) (38,666 Polymarket binary markets, Feb 2026 snapshots, seven LLMs): only two of seven models had
positive returns; Sharpe ratios were near zero (best 0.02, worst -0.22); stated confidence stayed around 0.8-0.9 even for
the losers; the most accurate model (Gemini-3-Flash, 75.0%) was not the most profitable; the paper chose a custom return metric over
proper scoring rules; the evaluation window was one week and order-book coverage 49.2%; its huge APY figures come from very short holding periods and mean little.
Reading: the field's agents are overconfident, unscored and unaudited. Calibration, abstention and honest scoring are open ground.
New ideas:
- I11 Calibration firewall: no model output reaches the selector until recalibrated on a pre-registered holdout; live ECE is tracked and a drifting model is demoted automatically.
- I12 Coverage and trading are separate: forecast the WHOLE committed universe for scoring (cannot cherry-pick), but trade only the subset with net edge. Publish both numbers.
- I13 Sim-to-live gap as a public metric: `mas-research::sim` is conservative by construction (queue-aware, never fills on doubt, fills at our price not the better print);
  we publish assumed versus realised fill rate and markout every month. Narrowing and disclosing that gap is itself a differentiator.
Built this round: `sim.rs` (queue-position fills, trade-through, markout, settlement, reward score), 6 tests.

## 3e. Competitor round 3: the two profitable models and the case studies (2026-10-31)
Sources: [PolyBench](https://arxiv.org/html/2604.14199v1), [Raven-Agent belief-to-trade study](https://arxiv.org/html/2607.03015v1),
a [secondary article on bot profits](https://ai-checker.webcoda.com.au/articles/ai-bots-polymarket-trading-profits-40-million-2026) (its own sourcing is weak).

PolyBench's two winners (lot size US$10, one week of data): MiMo-V2-Flash (accuracy 62.1%, return on invested capital 17.6%, Sharpe 0.02) and
Gemini-3-Flash (accuracy 75.0%, return 6.2%, Sharpe 0.01). What they share and where they differ:
- Both earned most on trades tagged `news_catalyst` (MiMo 50.0% on 46.7% of trades; Gemini 20.7% on 40.0%). MiMo's `value_bet` earned 19.2%; its `arbitrage` LOST 31.1%.
- Gemini was the more accurate and the better calibrated (stated confidence 0.855 vs accuracy 75.0%), yet earned less: profit depends on the price paid per win, not on accuracy.
- Both held alpha up to roughly US$100 per trade (MiMo to about US$500, Gemini eroded sooner); beyond that, order-book slippage removed it. Capacity is small.
- Politics was where confident calls paid; crypto produced severe losses and overconfidence.
- Caution: Sharpe 0.01-0.02 is statistically indistinguishable from zero, the window is one week, and the paper does not report skip rates. These are leads, not edges.

Raven-Agent (preliminary, 42 trades): forecast-only -10.7% ROI; edge-proportional sizing -55.5% (worst); selection + deterministic risk + quarter-Kelly +15.9% (95% CI +3.8% to +30.0%);
on a larger fused archive +5.9% with a CI that includes zero. Lesson: the trading layer, not the forecaster, produced the gain, and edge-proportional sizing is dangerous.

Case studies (all unverified social-media or secondary reports, with survivorship bias; we have NOT confirmed any): a bot said to earn US$2.2M in two months at a 74% win rate; a wallet said to turn US$313 into US$438,000 in a month;
algorithmic arbitrage said to total about US$40M between April 2024 and April 2025 (an IMDEA study as relayed by that article; top 10 arbitrageurs 21% of it, multi-outcome "NegRisk" markets 73%); the same article reports only 0.51% of users earning over US$1,000.
This is exactly the gap VeriChain fills: nobody can currently tell a real track record from a story. It also shows where measured money was: multi-outcome rebalancing arbitrage, which is latency-contested.

Pre-registered hypotheses (each a registry family, each with a kill rule; none is an expectation):
| ID | Hypothesis | Kill rule |
|---|---|---|
| H1 | Forecasts conditioned on a dated news catalyst beat the market price on Brier skill | p > 0.05 after 200 resolved markets |
| H2 | Cheap-entry payoff asymmetry: the same hit rate earns more when entries are priced low (after cost) | net return <= 0 on the low-price bucket after 200 trades |
| H3 | Politics and macro are tradable; crypto is excluded until separately proven | any crypto trade before its own S0 pass is a spec violation |
| H4 | Alpha survives only at small size (about US$100 per trade); capacity is a hard cap | realised slippage above the sim assumption by more than 25% pauses the strategy |
| H5 | Multi-outcome rebalancing arbitrage remains profitable net of latency losses | paper fill rate under 50% of the simulated rate |
Built this round: `sizing.rs` (shrink toward market, charge costs first, Kelly multiplier hard-capped at 0.25, per-trade and liquidity caps, time-normalised ranking), 6 tests.

## 3f. Round 4: the real reward economics (Polymarket docs read 2026-10-31)
Sources: [maker rebates](https://docs.polymarket.com/developers/market-makers/maker-rebates-program), [liquidity rewards](https://docs.polymarket.com/market-makers/liquidity-rewards).
Spec correction (I16): MAS v4 section 2 says taker fees apply only to 15-minute crypto markets. The current docs list taker fees across most categories
(fee rate 0.07 crypto; 0.05 sports, economics, culture, weather, other; 0.04 finance, politics, mentions, tech; geopolitics fee-free), and fund maker rebates of 15% (sports),
20% (crypto) or 25% (everything else) of taker fees. Amend section 2 and re-verify against the live docs before any build depends on it; Polymarket can change all of this at will.

Two income streams, both pro rata against rival makers:
1. Liquidity rewards: per-market daily pool; order score ((v-s)/v)^2; single-sided scores at 1/3 inside the 0.10-0.90 midpoint band and ZERO in the tails; the book is sampled once a minute at a random offset, up to 1,440 samples per UTC day; US$1 minimum payout.
2. Maker rebates: fee-equivalent C x rate x p x (1-p), so rebate income peaks at p = 0.5 and vanishes near 0 and 1.
Consequences for LP-1 (hypotheses, not promises):
- Quote UPTIME is income: a random minute-by-minute sample means a quote that is absent for part of the day forfeits proportionally.
- Market selection by reward yield = pool / rival score, per dollar at risk: crowded pools dilute to nothing; thin, uncrowded pools (long-tail markets) are where a small maker can earn. Competition density is unmeasured, so the first paper task is to MEASURE it.
- Reward income must beat adverse selection: yield is reported net of markout, never gross.
- Mid-band quotes can run one-sided at a third of the score to cut inventory risk; tail markets must be two-sided or earn nothing.
Built this round: `rewards.rs` (order score with min-size rule, Q_min with the c = 3 and tail rule, pro-rata expected reward, fee-equivalent and rebate, yield), 6 tests.

## 3g. Round 5: informed flow and calibration (2026-10-31)
Sources: [informed trading on Polymarket, arXiv 2605.02287](https://arxiv.org/pdf/2605.02287), [KalshiBench, arXiv 2512.16030](https://arxiv.org/html/2512.16030v1).
- Informed flow is the maker's main enemy. Literature screens find about 3.14% of accounts are "skilled winners" who capture over 30% of platform gains (with market makers), and
  that order imbalance from flagged accounts (created within 7 days of an event, trading only that event, dormant afterwards) predicts both next-period price changes (t = 2.54) and outcomes (t = 8.65).
  The flag has no published precision study and the labels and code are not released, so we must reproduce it ourselves. No real-time quoting signal is proposed there; ours is an inference.
- In our synthetic replay (`replay.rs`; plumbing check only) a few percent of informed taker flow erases a 2-cent half spread, and informed flow is worse than its share because it all hits the same side and consumes the queue first.
- KalshiBench (300 questions, five frontier models): every model was overconfident; ECE ranged 0.120 to 0.395; only one model had a positive Brier skill score over the base rate (0.057); at 90%+ stated confidence accuracy was 30.8% to 70.0%;
  more reasoning increased confidence without matching accuracy. The paper recommends post-hoc calibration and domain-specific validation.
New ideas, built this round:
- I18 Toxicity guard (`toxicity.rs`): recent flow becomes an imbalance score with young single-event wallets up-weighted; the action is Quote, Widen (1.5x to 3x) or Pull, and bad input fails SAFE (pull). Thresholds are paper-trading parameters.
- I11 Calibration firewall implemented (`recalibrate.rs`): Platt scaling fitted by Newton's method on a pre-registered holdout, refuses thin or one-class data, never outputs certainty; on an overconfident synthetic forecaster it recovers the shrinkage and improves held-out Brier.
- I19 Replay and S0 gate (`replay.rs`): a synthetic world through simulator, income and registry; the gate refuses losers, short histories and broken registries, and the same evidence clears a bar with an honest registry but not after 3,000 abandoned variants are disclosed. SYNTHETIC: it proves the plumbing, not an edge.

## 4. What this does not do
It does not guarantee profit, remove market risk, or make any strategy beat its costs. The worst case remains losing the
capped capital. A verified loss is still a credible, newsworthy record of how honest the system is.

## 5. Next build steps
mas-research: simulator with fee, reward and fill model; walk-forward harness; markout calculator; then paper trading
through `vc-sdk` into a `vc-seq` log so the first track-record pack exists on day one of paper trading.

## Round 7 (2026-11-02): breakout thesis, theory base, and what was built

Sources read this round: Optimal Market Making in Prediction Markets (arXiv 2607.17991, partial text), Unravelling the Probabilistic Forest (AFT 2025, arXiv 2508.03474), Adverse Selection in Prediction Markets: Kalshi (Bartlett, abstract only), Prophet Arena (ICLR 2026), Bayesian Linguistic Forecaster (arXiv 2604.18576). Formulas in `quoting.rs` are OUR adaptation of the paper's structure, not its published solution (the excerpt did not include the solution).

### Thesis
Profit alone will not differentiate us: faster, richer firms can always out-trade a US$5,000 account. What they cannot copy cheaply is a performance claim a stranger can verify. So the plan is a stack of (1) strategies whose risk is bounded by arithmetic rather than by belief, (2) statistics that cannot be gamed, and (3) a record that cannot be backfilled.

### Layers
1. Worst-case-bounded strategies (no forecast needed): `arb.rs` rebalancing arbitrage in exclusive/exhaustive sets; `implication.rs` implication arbitrage across rising-threshold chains (settlement payoff proven nonnegative in every legal outcome; only leg-fill risk remains and is haircut). Others extracted about US$40M on Polymarket in a year (AFT 2025), mostly by speed; our angle is breadth (many constraint chains), resting-maker execution, and refusing inferred dependencies (the study's LLM-detected dependencies were mostly false positives).
2. Market-making with prediction-market-aware quotes: `quoting.rs` (price-dependent volatility that vanishes at 0 and 1, settlement penalty, inventory normalised by the hard limit, post-only, longshot floor); `toxicity.rs` pulls quotes on one-sided flow (Kalshi study: toxicity predicts maker losses in single-name markets, not broad ones).
3. Forecasts only through a gate: `gate.rs` trades a forecast only if the Hoeffding lower bound of the forecaster's own bin frequency still beats price plus cost; overconfidence is measured, not trusted (Prophet Arena: models calibrated but too conservative; KalshiBench: reasoning raises confidence, not accuracy). `recalibrate.rs` Platt scaling feeds it.
4. Anytime-valid evidence: `eprocess.rs` (betting e-processes, Ville's inequality) lets capital be released in proportion to evidence and a failing strategy be killed early, with no peeking penalty; `registry.rs` hash-chains every trial so DSR (`lib.rs`) uses the true trial count.
5. Unforgeable record: forecasts and intents are committed to the witnessed evidence log before outcomes; `scoring.rs` rejects anything anchored after resolution; epoch seals expose a hidden tail. A track record becomes a product (verification, mandates) whatever the P&L.

### Hypotheses added (all pre-registered before any test; none measured)
H6 longshot floor improves maker P&L. H7 inventory-normalised skew reduces end-of-day inventory variance versus symmetric quotes at equal fill rate. H8 implication chains of three or more thresholds show executable violations at least weekly on liquid events. H9 the gate raises realised Sharpe of forecast-driven trades versus ungated, at fewer trades. H10 evidence-proportional capital release reaches the same capital as a fixed 60-day gate in less time when the strategy is truly profitable and never later when it is not.

### Not yet true
No venue connector, no real data, no paper P&L. Every number above is a design target until T0 replay and T2 paper results exist.

## Round 8 (2026-11-02): aggressive growth with provable safety rails

Sources read: Risk-Constrained Kelly Gambling (Boyd et al., abstract), Kelly theory (Thorp, MacLean-Ziemba; formula from memory of the literature and checked by Monte Carlo in the tests), the survey Agentic Trading (arXiv 2605.19337: 77 studies, only 19 closed-loop; no established real profitability; 15 of 19 not reproducible). Not read: "Trading Agents or Market Crashers?" (arXiv 2609.19705) and TradeTrap (fetch rate-limited); their headline claim, from titles only, is that LLM trading agents can be pushed into losses by corrupted inputs.

### What "aggressive" can honestly mean
Growth rate is capped by edge: at Kelly multiple k, log growth per period is (k - k^2/2) * SR^2 / 2 (SR in the same period). Full-Kelly annual growth is 12.5% at Sharpe 0.5, 50% at Sharpe 1, 200% at Sharpe 2. Sizing cannot manufacture edge; over-betting (k >= 2) gives zero or negative growth. So aggression is allowed only as far as evidence supports it: `growth.rs` sizes at the largest multiple whose probability of ever losing a chosen fraction of capital is bounded (k = 2 / (1 + ln(alpha) / ln(x)); for a 5% chance of ever halving k is about 0.38 and keeps about 61% of full-Kelly growth), then shrinks it by mu^2 / (mu^2 + se^2) for estimation error. The e-process ladder (`eprocess.rs`) raises the capital ceiling only as evidence accumulates, so aggression is earned.

### AI-backed safety, with the AI kept out of the loop that enforces it
`shield.rs` is a deterministic gate between any proposal (LLM, statistical, human) and an order: per-order, per-market, per-event and total caps; daily loss limit; sticky drawdown kill; stale-data refusal; disputed-market refusal; no new exposure near resolution; order-rate limit; non-finite or impossible state is denied; risk-reducing orders are always allowed. A 20,000-case randomized test checks no allowed order can breach a cap. AI is used where it is strong (forecasting, anomaly detection, market selection) and never where a mistake is unbounded. Survey-derived rules to build into mas-risk and the agent harness: read-only ground-truth state written only by the execution environment; outcome embargo (no retrieval of an episode's outcome until its window ends); point-in-time data with modelled ingestion lag; provenance and version stamps on every retrieved fact; full logging of prompts, tool calls and failed branches into the evidence log; curated sources only for anything that can move size.

### Capacity, the real wall
Even a perfect edge is capped by what the book will absorb. At US$5,000 capital the limit is risk policy, not capacity; scaling beyond requires depth data (T0) to know how much each market can take before our own impact erases the edge. Any growth claim must therefore state capacity per strategy.

### Hypotheses added (pre-registered, unmeasured)
H11 drawdown-bounded sizing keeps realised maximum drawdown inside its bound in at least 95% of paper-trading paths at equal or better growth than fixed quarter-Kelly. H12 the shield changes no outcome for honest proposals (denies fewer than 1% of orders) while stopping 100% of injected over-limit proposals in red-team replays.

## Round 9 (2026-11-02): real-data pipeline (mas-data)
Built: strict parsers for the CLOB order book (GET /book, OrderBookSummary: market, asset_id, timestamp, hash, bids, asks, min_order_size, tick_size, neg_risk, last_trade_price; all numbers are strings) and Gamma events (clobTokenIds and outcomes may be JSON text inside a string); crossed, malformed, out-of-range and over-deep books are refused; ordering is never trusted. Recordings are hash-chained JSONL (raw response bytes kept verbatim) with a head hash to commit to the evidence log before analysis. `mas-data scan` runs the rebalancing-arbitrage detector over complete negRisk events per round. CLI tested end to end against a local fake of both endpoints; real endpoints untested from this environment (no network route).
Gaps recorded: (1) a public trades feed is needed to replay FILLS (books alone cannot validate queue-position and markout assumptions in sim.rs); (2) implication chains need a human-curated chain file (threshold ladders), never inference; (3) Gamma query parameters (active, closed, limit, order, ascending) are from documentation memory, to be confirmed on first run; (4) the 120 ms politeness delay is a guess pending rate-limit documentation.
Protocol: the first recording is pre-registered in the trial registry (what will be measured: frequency and size of rebalancing candidates after fees and haircut, by event type) before it is analysed.

## Round 10: claiming it (the Verified Performance Card)

"Most profitable and safest on the planet" is a superlative: no check can prove it, so we never print it. What we CAN do
is make every statement about performance a recomputable fact, and invite anyone to beat it.

`mas-research::card` builds a card as a pure function of (return ledger, trial registry, audit facts, log head):
- Performance claims: net log return; Sharpe lower confidence bound (skew/kurtosis adjusted); deflated Sharpe over the
  hash-chained trial registry (hidden failed trials are detectable and deflate the claim); anytime-valid e-process
  profit evidence (valid under continuous peeking).
- Safety claims: every order had a logged intent first; capital cap respected with zero overrides; drawdown within the
  shield bound; kill switch tested recently; books reconcile to the venue; minimum live days.
- Standing: Unverified (safety missing) / Candidate (safety proven, performance not) / Verified (all proven).
- Canonical JSON, SHA-256 `card_id`, `verify_card` recomputes it byte for byte; `public_statement` states only proven
  facts, lists what is NOT yet established, and ends with the challenge: publish a card computed the same way.
Status today: no live or paper ledger exists, so no card can be Verified. The machinery is ready for the first one.

## Round 11: the ledger the card needs (paper engine, trades recorder)

- `mas-research::paper`: maker-buy-only paper engine. Intent before order; hard exposure cap that cannot be exceeded;
  pessimistic fills (only a later public trade at or below our price, at most `queue_factor` of its size, charged at
  our price; no rebates or rewards counted). Every action is an `Ev`; `audit` and `returns` recompute the card's audit
  facts from the event list alone, so a log with an intent removed or an override added is caught.
- `mas-data trades`: hash-chained recording of public trade prints (data API), plus `parse_trades`/`merge_trades`.
  Parser is strict but UNVERIFIED against the live API (no network route here); the CLI subcommand has no fake-server
  test yet. First run in a Codespace is the validation.
- Pre-registration procedure: before the first paper day, `Registry::register("lp1", <canonical params text>)`, commit
  the registry head to the evidence log, only then start. Not yet automated.
- Still missing for a first card: replaying recorded books and trades through the engine with the quote model, real
  reconciliation (paper mode has no venue to reconcile with), and 90+ days of recording.

## Round 12: run it from the dashboard

- `mas-data::runner`: background collector (discover, then books and trades into hash-chained files), injectable
  network function, interruptible sleeps, per-session disk cap, drop guard so status can never stick at "running".
- `vc-server`: admin-only `/admin/v1/trading/{start,stop,status}`, `VC_COLLECT=1` to start at boot, and an operator panel
  in the console (status, start/stop, files, scan on demand, claim card). Tested through the HTTP API with a fake market
  and in a real browser against the real binary.
- Bug found by the new API test and fixed: the card proved "capital cap respected" and "drawdown within bound" on an
  EMPTY ledger. Both now require data (orders exist; enough periods). Regression test: an empty ledger proves nothing.

## Round 13: replay and honesty about paper

- `mas-data::replay` feeds recorded books and public trades through the paper engine (baseline join-the-bid policy, pessimistic
  fills, marked to mid) and builds the card; `mas-data paper` and the console button run it.
- Card rule added: `books_reconcile_to_venue` needs at least one actual reconciliation against a venue. Paper has none, so a paper
  card can never read Verified. This is deliberate: investors should see that the standard cannot be passed by simulation.
