# Multi-tenant Compliance Hub (v2) — design

**Date:** 2026-07-16 · **Status:** approved (pre-implementation) · **Supersedes** the per-token factory approach (SP1 `feat/constella-factory`, parked)

## Goal

A **single shared, multi-tenant** compliance stack so any issuer launches a real compliant token in **one signed transaction**, and every token is served by the same audited contract instances instead of a fresh ~10-contract copy per launch. This is the production-shaped foundation for the no-code issuer platform.

### Why this replaces the factory (SP1)

SP1 deployed a full per-token stack (dispatcher + identity + token + each module = ~10 contracts). Measured on testnet: a launch fits one tx only for **≤5 modules** (6–7 hit `HostError: Budget ExceededLimit`), and each issuer pays rent on ~10 contracts. Multi-tenant removes both limits: shared modules mean a launch deploys only the token (+ identity), so it is always well within budget and always one signature.

### Non-goals

- Rewriting `demo-token` or `identity-mock` — reused unchanged (the token already calls `hub.can_transfer(..., token)`; identity is deployed per issuer).
- The wizard UI / client-side deploy (later sub-project) and the Direction-B reskin.
- A real KYC provider (issuer attests via `identity-mock`, as before).
- Deleting the v1 single-tenant library — it stays as v1, documented (see §Rollout).

## Context

- **The hook ABI is already token-aware.** `crates/compliance` passes `token: Address` into every hook (`can_transfer(from, to, amount, token)`, etc.) and forwards it to modules. So the *public* hook surface does not change under multi-tenancy — only internal keying (`Modules(token, hook)`, `Admin(token)`) plus a bulk `configure`.
- **v1 modules are single-tenant:** they key state without the token (`module-max-balance` uses `Bal(holder)`; max-holders/max-investors/transfer-window similar). Sharing one instance across tokens would cross-contaminate — hence token-keyed module variants for v2.
- **A live v1 vulnerability (drives Phase 0):** `Compliance::transferred/created/destroyed` (`crates/compliance/src/lib.rs:98-117`) are public with **no auth**, and modules' post-event methods have none either. An attacker can call `compliance.transferred(victim, attacker, huge, token)` to corrupt the stateful modules' balance mirrors / holder counts, then **bypass caps or break counters**. Multi-tenancy would make this cross-issuer. The fix (post-events require the token's auth; modules accept calls only from the hub) is the same auth pattern v2 is built on — so it lands in v1 first.

## Phase 0 (prerequisite): patch the v1 post-event auth hole

Small, isolated, its own PR (`fix/v1-post-event-auth`), TDD:
- `Compliance::transferred/created/destroyed` call `token.require_auth()` — only the token contract can report its own events.
- Each stateful module's post-event methods require the dispatcher's auth (the dispatcher address, stored at construction, `require_auth()`), so a module accepts bookkeeping only from its dispatcher.
- Tests: a spoofed post-event from an arbitrary caller panics; the normal token→dispatcher→module flow still passes (extend `demo-token`'s integration test).

This pattern (post-events are authenticated) is the security backbone reused throughout v2.

## v2 Architecture

New crates; v1 crates untouched:
- **`crates/hub`** — the multi-tenant dispatcher. Per-token: admin, identity, per-hook module registry, and the typed issuer forwarders. One deployed instance serves all tokens.
- **`crates/hub-module-*`** (7) — token-keyed variants of the modules. State keyed by `(token, ...)`; parameters mutated only via the hub.
- **Reused unchanged:** `demo-token` (calls `hub.can_*`/post-events with its own address as `token`), `identity-mock` (deployed per issuer).

### Data model (hub)

```
Admin(token)          -> issuer (the token's admin)
Identity(token)       -> that token's identity provider (if any)
Modules(token, hook)  -> Vec<Address> of module instances for that hook
HubAdmin              -> platform admin (sets the wasm hashes)
WasmHashes            -> token + identity wasm hashes (learned from SP1: deploy instances from installed hashes)
```

### One-signature launch

```
launch(config) -> LaunchResult   // always 1 signature
```
`launch`: `config.admin.require_auth()`, deploy the token (+ identity iff an identity-dependent module is selected) via `deployer().deploy_v2(hash, args)`, write `Admin(token)`/`Identity(token)`, and for each selected module write `Modules(token, hook)` + initialize that module's per-token parameters (via the shared module instances, which the hub already knows). Budget: ≤2 deploys + parameter writes — far under the measured 5-module ceiling, so no staged API is ever needed.

### Module auth chain (structurally closes the v1 hole)

- Module state is token-keyed: `Bal(token, holder)`, `Denied(token, account)`, `Paused(token)`, `Count(token, country)`, etc.
- A module stores the hub address at construction; every mutator and post-event requires the hub's auth (`hub_addr.require_auth()`). Modules never trust an arbitrary caller.
- The hub's post-events require `token.require_auth()` — only the token reports its own events; the hub then fans out to the token's modules. Spoofing another token's state is impossible by construction.
- The public hook ABI (`can_transfer(from, to, amount, token)`, …) is unchanged — OZ-portability narrative preserved.

### Issuer surface: typed forwarders on the hub (single auth surface)

The issuer talks only to the hub: `hub.pause(token)`, `hub.unpause(token)`, `hub.add_to_denylist(token, account)`, `hub.remove_from_denylist(token, account)`, `hub.set_max_balance(token, cap)`, `hub.set_country_allow(token, codes)`, `hub.set_country(token, account, code)` (attest), etc. Each does `Admin(token).require_auth()` **once**, then calls the module's token-keyed setter. Authorization logic lives in exactly one contract; modules carry none beyond "caller is the hub". The wizard/SDK sees one address and one ABI.

## Testing

- **TDD, isolation-first:** every module variant and the hub tested with **two tokens** configured differently — assert zero cross-talk (token A's denylist/pause/cap never affects token B's mirror or decisions).
- **Negative auth:** configuring/mutating another issuer's token panics; a spoofed post-event (non-token caller) panics; a module mutator called by a non-hub caller panics.
- **End-to-end:** `launch` → attest → mint → transfer, asserting pass and rule-violating revert, per token.
- **Testnet:** deploy the hub once, install token/identity wasm, run a real `launch` from a funded issuer (prove **1 signature**), drive live pass/revert, capture tx hashes as evidence.

## Rollout / decomposition

Too large for one plan — sub-projects, each its own spec→plan cycle where noted; this spec covers the whole v2 design and the first plans:
1. **Phase 0** — v1 post-event auth patch (own PR, do first; establishes the auth pattern).
2. **Hub core + one module (denylist)** — the hub, its data model, `launch`, and one token-keyed module end-to-end with two-token isolation tests. De-risks the pattern (mirrors SP1's staging: prove the mechanics on the simplest module first).
3. **Remaining 6 token-keyed modules** — port each to token-keyed state + hub-auth, wire into `launch` + forwarders.
4. **Testnet measurement + evidence** — prove one-signature launch live.

Later sub-projects (own specs): the wizard UI, the Direction-B reskin.

### Parked / preserved

- **v1 single-tenant library** stays on `main`, unchanged, documented as "v1 (per-token, ERC-3643-style isolation)". A short doc will contrast v1 vs v2 (isolation vs shared-instance; when each fits).
- **`feat/constella-factory` (SP1)** is parked, not merged, not deleted. Its testnet measurements (the ≤5-module one-tx ceiling, the budget limit) are captured in this spec's rationale. It is not part of v2.
