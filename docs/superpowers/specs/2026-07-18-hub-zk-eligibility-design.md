# Private (ZK) Eligibility for Launched Tokens — Design

**Date:** 2026-07-18
**Depends on:** the completed multi-tenant `Hub` (7/7 modules) and the existing standalone ZK stack (`crates/zk-verifier`, `crates/module-identity-zk`, `crates/module-zk-eligibility`, the `country_eligibility.circom` circuit + browser proving in `web/src/zk/*`).

## Goal

Make **country eligibility private** for launched tokens. Today the hub's `country_restrict` reads a holder's country in cleartext (the issuer attests it via `set_country`). This adds a **private variant**: at launch the issuer can choose "ZK proof" instead of cleartext, and each holder proves in their browser that their (hidden) country is in the allowed set. The country is never written on-chain — only a proof of eligibility. This wires the already-built ZK stack into the multi-tenant hub and into the real product flow (launch wizard + token console), and removes the standalone `/zk` demo.

## Non-goals

- No new circuit. The existing `CountryEligibility(2)` circuit is fixed to **exactly 2 allowed countries** — private tokens allow up to 2 jurisdictions (pad a single choice by repeating it). Cleartext `country_restrict` stays unlimited.
- ZK eligibility and country-based `max_investors` are **mutually exclusive on one token** — the ZK identity returns `country_of = None` by design, so a per-country cap cannot read a country. The wizard prevents combining them.
- The standalone `/zk` demo page is removed from the product (its code stays in git history for the Instaward evidence).

## Architecture

### The existing ZK pieces (reused, not rebuilt)

- `zk-verifier` — Groth16/BLS12-381 verifier: `verify_proof(vk, proof, signals) -> bool`.
- `module-identity-zk` — per-token identity: `set_policy(vk, allowed)`, `register_self(account, commitment)` (wallet-authed), `prove_eligibility(account, commitment, proof) -> bool`, `is_verified(account) -> bool`, `country_of -> None`. Same `IdentityProvider` ABI as `identity-mock`.
- The circuit proves `commitment == Poseidon(country, secret)` and `country ∈ allowed[2]`. Public signals = `[commitment, allowed[0], allowed[1]]`.

### Shared-type refactor (enabling the hub to handle the VK)

`VerificationKey` and `Proof` currently live in `zk-verifier` (a `#[contract]` crate). The hub is a `#[contract]` and must not depend on another `#[contract]` (duplicate-`__constructor` link error). So **move `VerificationKey` and `Proof` into `crates/module-interface`** (the shared no-contract types crate); `zk-verifier` and `module-identity-zk` import them from there. The `#[contracttype]` layout is unchanged, so every existing on-chain ABI stays identical. The hub then imports `VerificationKey` from `module-interface`, stores it once (platform config), and passes it to each per-token identity's `set_policy` via a `#[contractclient]` `IdentityZkAdminClient`.

### New hub module: `hub-module-zk-eligibility`

Token-keyed variant of `module-zk-eligibility`, mirroring the other `hub-module-*` crates:
- `__constructor(hub)`.
- `configure(token, identity)` — hub-authed; stores `Identity(token)`.
- `can_create(to, _amt, token)` / `can_transfer(_from, to, _amt, token)` — return `IdentityClient::is_verified(to)` reading `Identity(token)`. (Checks the recipient, matching the single-tenant module.)
- post-events (`created`/`transferred`/`destroyed`) — no-op (stateless gate).
- read `identity(token) -> Address`.
- `require_hub` on `configure`.

`module-interface` gains a `ZkEligibilityClient` trait (`configure`, `identity`) and an `IdentityZkAdminClient` trait (`set_policy(vk, allowed)`) that the hub calls on the per-token identity.

### Hub wiring

**Platform config (platform-admin only):** `set_verifier(addr)`, `set_zk_identity_wasm(hash)`, `set_zk_vk(vk)`. New `DataKey::{Verifier, ZkIdentityWasm, ZkVk}`.

**`LaunchConfig`** gains `zk_eligibility: bool` (the last field; the ScMap sort places `zk_eligibility` after `transfer_window`).

**`launch`** — when `zk_eligibility` is true (requires `country_restrict` non-empty as the allowed set; the launch uses it as the ≤2-country policy):
1. Deploy a per-token `module-identity-zk` via `deploy_v2(zk_identity_wasm, (admin, verifier))`.
2. Call `IdentityZkAdminClient::set_policy(stored_vk, country_restrict)` on it.
3. Store `Identity(token) = zk_identity`.
4. Register `hub-module-zk-eligibility` on `CanCreate` + `CanTransfer`, `configure(token, identity)`.
5. **Skip** the cleartext `country_restrict` block and the `max_investors` identity block for this token (mutual exclusion). A ZK token's `Identity(token)` is the ZK identity.

When `zk_eligibility` is false, launch behaves exactly as today (cleartext identity + country_restrict / max_investors).

**Reads/forwarders:** `is_verified(token, account) -> bool` (passthrough to the token's ZK identity) for the console. Attestation itself is not forwarded — holders call `register_self`/`prove_eligibility` on `hub.identity(token)` directly (same pattern as cleartext attestation calling `set_country` on the identity directly).

### Bootstrap (`scripts/bootstrap-hub-testnet.sh`)

Also deploy the shared `zk-verifier`, upload the `module-identity-zk` wasm (record its hash), compute the VK via `tools/zk-encode`, and set `set_verifier` / `set_zk_identity_wasm` / `set_zk_vk` on the hub. Record `verifier` + `zkIdentityWasm` in `web/src/hub.testnet.json`.

### Frontend

- `web/src/zk/prove.ts` — `generateProof(country, secret, allowed: string[])` (parametrize the allowed set instead of the hardcoded `['840','276']`).
- `web/src/hub.ts` — `LaunchConfig.zk_eligibility` in the type + `launchConfigScVal` (new sorted key) + `blankConfig`; a `proveEligibility(token, account, country, sign, onStep)` that reads `hub.identity(token)` + its `allowed()` set, generates the browser proof, and submits `register_self` + `prove_eligibility` on that identity; `readIsVerified(token, account)`.
- **Launch wizard** — the "Country restrict" module gains a **"Private (ZK proof)"** switch; enabling it sets `zk_eligibility = true`, limits the country picker to 2, and disables `max_investors` (with an inline note). The constellation shows a `zk` star.
- **Token console** — for a ZK token, replace the cleartext "Attest identity" panel with a **"Prove eligibility (ZK)"** panel: the connected wallet picks its (private) country, generates the proof in-browser with the **live 4-step pipeline** (moved out of the demo), and submits — the console shows `is_verified` flipping true. Minting to a proven holder passes; to an unproven holder it's rejected on-chain.
- **Remove the `/zk` demo** — delete the route, the nav/docs references, and `LegacyDemo.tsx`; keep `zk/prove.ts` + `zk/encode.ts` (now used by the console). The ZK proof pipeline UI moves into the console.

## Testing

- Contract: `hub-module-zk-eligibility` unit tests (gates on a mock ZK identity's `is_verified`; hub-auth guard). Hub e2e: a two-token test where token A is ZK (proven holder passes, unproven denied) — using the `module-identity-zk` + `zk-verifier` wasm via `contractimport!`, driven by the golden proof from `tools/zk-encode`.
- Frontend: `tsc --noEmit` + `vite build` clean; `verify:launch` golden updated for the new `zk_eligibility` field.
- **Live testnet:** bootstrap; launch a ZK token (`country_restrict:[840,276], zk_eligibility:true`); an account registers + proves eligibility (browser or CLI with the golden proof) → `is_verified` true → mint passes; an unproven account → mint rejected. Record tx hashes.

## Build order

1. Move `VerificationKey`/`Proof` to `module-interface` (refactor; ZK crates still build + pass).
2. `hub-module-zk-eligibility` module (TDD).
3. `module-interface`: `ZkEligibilityClient` + `IdentityZkAdminClient` traits.
4. Hub wiring: platform config + `LaunchConfig.zk_eligibility` + launch branch + `is_verified` read (+ golden/encoder + migrate LaunchConfig literals).
5. Bootstrap: verifier + zk-identity wasm + VK; update `hub.testnet.json`.
6. Frontend: prove.ts param, hub.ts prove flow, wizard toggle, console ZK panel, remove demo.
7. Live testnet verification + evidence.

---

## Live testnet evidence — private (ZK) eligibility

Verified end-to-end against the bootstrapped hub `CCY5WSFHF5ZROF3IPVZAXNSTT63CPJWITYG36VBOK2JMKF7UX5HSZWBC`:

- **One-signature launch** of a ZK token (`country_restrict:[840,276], zk_eligibility:true`): tx [`843c82b5…`](https://stellar.expert/explorer/testnet/tx/843c82b58eba87ba322421a0a0ce29063b5538bc1056cc9f2d4a6ccf066d7a1f) → token `CC6W6UJVJCI7MFWGSOEFP3UDKSTIVDWK6ULBI7HKSOWLGEP73CAKVDSR`. The hub deployed a per-token ZK identity `CB6EP4RCGOY3XQRAYFROI7GAOHIAHVWLQ5G3V6IOCYMGM6DH4EMM5N47` and set its policy (VK + allowed `{840,276}`).
- A holder proved eligibility with a browser-style Groth16 proof (`register_self` + `prove_eligibility`) — `hub.is_verified(token, dave) = true`; an un-proven account `hub.is_verified(token, eve) = false`. **The country was never submitted or revealed.**
- Minting to the proven holder **passed** (balance 10); minting to the un-proven holder **reverted** on-chain — private eligibility enforced by the hub, exactly like every other rule.
