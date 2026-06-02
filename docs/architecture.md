---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7]
inputDocuments: ['docs/PRD-Constella.md']
workflowType: 'architecture'
project_name: 'Constella'
user_name: 'Berkay'
date: '2026-06-03'
communication_language: 'Turkish'
document_output_language: 'English'
authoredMode: 'autonomous (decisions logged in docs/DECISIONS.md)'
---

# Architecture Decision Document — Constella

_Open-source, modular compliance infrastructure for Stellar RWA tokens (Soroban). Input: `docs/PRD-Constella.md`. Decisions: `docs/DECISIONS.md`._

> Authored in autonomous mode. The collaborative A/P/C gates of the workflow were superseded by an explicit grant of decision authority; key choices are logged in `DECISIONS.md`.

---

## 1. Project Context Analysis

### Requirements overview

**Functional (from PRD):**
- A **standard compliance module interface** that modules implement (the core deliverable).
- A **compliance dispatcher** that registers modules per hook and runs them on transfer (mirrors OZ's engine).
- A **library of 4 modules**: MaxHolders, Lockup, MaxBalance, CountryRestrict.
- A **mock identity provider** (attestor stand-in) for the identity-dependent module.
- A **demo**: deploy a permissioned token + register modules + a real testnet transfer that passes / reverts; surfaced via a thin Web UI ("launch-taste").

**Non-functional:**
- **Security/correctness first** — this gates value transfer; modules must be auditable, small, single-purpose.
- **Composability** — modules add/remove without touching the token (modular compliance).
- **Trust model** — self-contained modules trustless (on-chain state only); identity module depends on an attestor (correct for compliance).
- **Resource budget** — stay within Soroban per-tx limits (100M instr); ZK verification (~40M) is Phase 2.
- **Portability** — modules should work for any Soroban permissioned token, not only one issuer.

**Scale & complexity:** medium. Primary domain: Soroban smart contracts (Rust) + a light dApp. Cross-cutting concerns: storage/TTL management, admin/authorization, event emission, the module ABI contract between dispatcher and modules.

### Technical constraints & dependencies
- `soroban-sdk` (pinned). OZ `stellar-tokens` is the production target but its module ABI is unpublished → see D3.
- Soroban storage tiers (instance/persistent/temporary) + TTL bumping.
- Testnet via `stellar` CLI + friendbot.

### Cross-cutting concerns
- **Authorization:** who can register modules / set module config (admin via `require_auth`).
- **Storage & TTL:** module configs and counters are persistent; bump TTL on writes.
- **Events:** emit on module decisions for observability/forensics.
- **The dispatcher↔module ABI:** the contract that makes modules pluggable.

---

## 2. Starter / baseline

No starter template. Greenfield Cargo workspace, scaffolded with `stellar contract init` conventions. Boring, standard Soroban layout.

---

## 3. Key architecture decisions

### 3.1 Tech stack
- **Contracts:** Rust + `soroban-sdk`, Cargo workspace (one crate per module + interface + mock identity + demo dispatcher/token + integration tests).
- **Build/deploy:** `stellar` CLI → testnet (friendbot).
- **Web UI:** React + Vite + TS + `@stellar/stellar-sdk` + Freighter.
- **Tests:** `cargo test` with `soroban_sdk` testutils (unit + integration), negative-path tests, then a live testnet demo script.

### 3.2 The module ABI (core contribution)
A compliance module is a separate contract the dispatcher calls. Functions a module MAY export (implement only what it needs):

```
can_transfer(env, from: Address, to: Address, amount: i128, token: Address) -> bool
can_create(env, to: Address, amount: i128, token: Address) -> bool
transferred(env, from: Address, to: Address, amount: i128, token: Address)   // post
created(env, to: Address, amount: i128, token: Address)                       // post
destroyed(env, from: Address, amount: i128, token: Address)                   // post
```

Plus per-module: `init/config` (admin-set parameters) and `__admin` (via `require_auth`). Mirrors OZ's `ComplianceHook` set { CanTransfer, CanCreate, Transferred, Created, Destroyed } so it is portable to OZ's dispatcher later (D3).

### 3.3 The dispatcher (Compliance engine)
- Storage: `Map<ComplianceHook, Vec<Address>>` of registered module addresses.
- `can_transfer(...)`: loop CanTransfer modules, call each module's `can_transfer`, **AND-combine**; any `false` ⇒ deny.
- `transferred(...)`: call each Transferred module (state updates).
- `add_module_to(hook, module)` / `remove_module_from(hook, module)` (admin, `require_auth`).
- For MVP we ship our own minimal dispatcher (D3); it intentionally matches OZ's surface.

### 3.4 Identity boundary
`IdentityProvider` interface: `country_of(addr) -> Option<u32>` (and room for `is_verified`, `attribute_of`). MVP implementation = **mock registry** (admin sets `address → ISO-3166 numeric`). The ZK variant (Phase 2) implements the same interface backed by a Groth16-verified eligibility flag. Modules depend on the interface, never on the implementation.

### 3.5 The four modules
- **MaxHolders** (stateful): config `max`. Pre: if `balance_of(to)==0` (new holder) and `holders >= max` ⇒ deny. Post `transferred`: if `to` went 0→+ increment; if `from` went +→0 decrement. Holder count + max in persistent storage.
- **Lockup** (stateful, time): config `unlock_ledger`/duration. Pre: deny `from` transfers while `ledger.timestamp < unlock`. Post `created`/`transferred`: record acquisition time per holder.
- **MaxBalance** (stateless): config `max_per_holder`. Pre: deny if `balance_of(to) + amount > max`.
- **CountryRestrict** (identity-dependent): config allow-list of country codes. Pre: `country_of(to)` must be in allow-list (queries `IdentityProvider`).

Balance reads use the token's SEP-41 `balance(addr)` via cross-contract call.

### 3.6 Demo permissioned token
Minimal SEP-41-style fungible token whose `transfer`/`mint` call the dispatcher's `can_transfer`/`can_create` before moving balances and `transferred`/`created` after (mirrors OZ RWA token flow). Demo-only; production uses OZ's RWA token.

---

## 4. Patterns & conventions
- **Storage:** persistent for configs/counters; bump TTL on write; instance for admin/wiring.
- **Auth:** admin ops `require_auth`; modules are configured by their admin, registered by the dispatcher admin.
- **Errors:** `#[contracterror]` enums; pre-checks return `bool` (dispatcher decides), hard failures panic with typed errors.
- **Events:** emit `("compliance","denied"/"passed", module, from, to)` for observability.
- **Module independence:** each module is a standalone crate, independently testable, single-purpose.

---

## 5. Repository structure

```
constella/
├── Cargo.toml                      # workspace
├── crates/
│   ├── module-interface/           # ComplianceModule + IdentityProvider traits, shared types
│   ├── compliance/                 # dispatcher (engine) — mirrors OZ surface
│   ├── module-max-holders/
│   ├── module-lockup/
│   ├── module-max-balance/
│   ├── module-country-restrict/
│   ├── identity-mock/              # attestor stand-in (IdentityProvider impl)
│   └── demo-token/                 # minimal SEP-41 permissioned token (demo)
├── tests/                          # integration tests (token + dispatcher + modules)
├── scripts/                        # testnet deploy + demo transactions (stellar CLI)
├── web/                            # React+Vite launch-taste UI
└── docs/                           # PRD, architecture, decisions
```

---

## 6. ZK extension (Phase 2 — seam only)
`module-identity-zk` (or a ZK-backed `IdentityProvider`): investor pre-registers a Groth16 proof (BLS12-381, CAP-0059) that a trusted-issuer-signed credential satisfies a predicate; verifier stores an eligibility flag; `CountryRestrict`/eligibility modules read the flag via the same `IdentityProvider` interface. No MVP code; §3.4 boundary is the integration point. Generic verifier + trusted-issuers-registry (issuer pubkey as public input); credential format standardized (one SNARK-friendly scheme to start).

---

## 7. Test & validation strategy
- **Unit:** per module via `soroban_sdk` testutils `Env` — positive + negative (denied) paths, boundary conditions.
- **Integration:** wire demo-token + dispatcher + modules; assert compliant transfer passes, non-compliant reverts; multi-module composition.
- **On-chain:** `scripts/` deploy to testnet (friendbot), run real pass/revert transfers, capture explorer links for the demo.
- **Gate before "done":** `cargo build` + `cargo test` + `stellar contract build` all green; a real testnet transfer observed.

---

## 8. Build order (epics)
1. `module-interface` (traits/types) → 2. `compliance` dispatcher → 3. `identity-mock` → 4. four modules → 5. `demo-token` → 6. integration tests → 7. testnet deploy scripts → 8. web UI → 9. (Phase 2) ZK.
