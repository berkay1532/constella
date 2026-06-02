# Decision Log — Constella

> Autonomous-mode decision log. Each entry is a choice I made on Berkay's behalf (he asked not to be interrupted until the demo is ready), with rationale. Items marked **[CONFIRM LATER]** are things I would normally have asked about — review them when you're back.

## Format
`#` · Decision · Rationale · Status

---

### D1 — License: **Apache-2.0**
Permissive, common for infra libraries, broad adoption. To be aligned with OpenZeppelin `stellar-contracts` licensing. **[CONFIRM LATER]** (verify OZ stellar-contracts license; switch to match if needed.)

### D2 — Language/SDK: **Rust + soroban-sdk** (Cargo workspace, multi-crate)
Native Soroban contract language. Workspace = one crate per module + interface + mock identity + demo dispatcher/token + integration tests. Matches PRD repo sketch.

### D3 — MVP integration approach: **self-contained reference implementation** (our own minimal compliance dispatcher + minimal permissioned token), mirroring OZ's hook model — NOT direct integration with OZ's exact dispatcher.
Why: OZ ships the Compliance dispatcher but does **not** publish a standard module interface/ABI; reverse-engineering their exact module call is a blocker risk. For a buildable, demonstrable MVP I implement a clean dispatcher + module trait that mirrors OZ's `ComplianceHook` model (`can_transfer`/`transferred`/`can_create`/`created`/`destroyed`). Designed to be ABI-portable to OZ later. **[CONFIRM LATER]** (align with OZ module ABI once verified / coordinate with OZ/Association — this is also a §8 validation item.)

### D4 — Module interface (our core contribution): a `ComplianceModule` contract interface
Hooks a module may implement: pre-checks (`can_transfer`, `can_create` → bool) and post-hooks (`transferred`, `created`, `destroyed`). Modules implement only the hooks they need + own config storage + admin. Dispatcher AND-combines pre-check results across registered modules.

### D5 — MVP module set: **MaxHolders, Lockup, MaxBalance, CountryRestrict**
Per PRD §6. 3 self-contained (trustless) + 1 identity-dependent. Stateless + stateful mix to prove the interface generality.

### D6 — Identity layer: **mock attribute registry** (admin-set `address → country/attrs`)
Represents the attestor/issuer role. Pluggable behind an `IdentityProvider` interface so a real provider (idOS/attest) or the ZK variant can replace it. We do NOT build a real KYC layer.

### D7 — Demo token: **minimal SEP-41-style permissioned fungible token** that calls the dispatcher on transfer
To show the end-to-end flow (transfer → compliance check → pass/revert) without depending on OZ's unpublished module ABI. In production this is OZ's RWA token.

### D8 — Network: **testnet** for the live demo (friendbot-funded)
Free, real on-chain transactions, no user keys/funds needed. Mainnet is post-MVP.

### D9 — Web UI stack: **React + Vite + TypeScript + @stellar/stellar-sdk + Freighter**
For the "launch-taste" demo: deploy/select modules + run a real transfer that passes/reverts. Boring, standard Stellar dApp stack. **[CONFIRM LATER]** (UI look/feel — Kaan/UX could refine.)

### D10 — Git workflow: **GitHub Flow** — feature branches → PR → squash-merge → main
main always green. Repo: `github.com/berkay1532/constella` (private). Only this repo is touched.

### D11 — ZK layer: **out of MVP**, Phase 2 (`module-identity-zk`)
BLS12-381 Groth16 (CAP-0059), pre-registered eligibility flag pattern. Not built in MVP; architecture leaves a clean seam (D6 IdentityProvider interface).

### D12 — SCF track (Build vs Public Goods): **deferred**
Per PRD §13 — decide with mentor after hackathon. No code impact.

---

## Open questions I would have asked (for your review later)
- Q1: Confirm Apache-2.0 vs OZ's actual license. (D1)
- Q2: OK to ship a self-contained reference impl now and align with OZ's module ABI later? (D3)
- Q3: Demo UI design/branding preferences. (D9)
- Q4: Which exact country codes / lockup periods / caps to showcase in the demo (using sensible defaults: ISO-3166 numeric, e.g., allow {US=840, DE=276, TR=792}; lockup 60s for demo; maxHolders=3; maxBalance=40% of supply). 
- Q5: SCF track choice. (D12)
