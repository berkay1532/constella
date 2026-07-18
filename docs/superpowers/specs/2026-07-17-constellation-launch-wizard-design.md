# Constellation Launch Wizard + Token Console — Design (SP2)

**Date:** 2026-07-17
**Status:** Approved (design), pending spec review → implementation plan
**Depends on:** the completed multi-tenant `Hub` (7/7 modules, `crates/hub`, all proven live on testnet).

## Goal

A no-code web experience where anyone connects a Freighter wallet and, in **one signed transaction**, launches their **own real compliance token** on Stellar testnet — choosing which of the 7 compliance modules apply and configuring each — then exercises that token from a console to **see the restrictions enforce live** (mint, attest country, transfer, watch compliance reject). No mocks: real `hub.launch(LaunchConfig)`, real modules, real on-chain rejections.

## Non-goals

- No changes to any Soroban contract. The hub is complete (7/7) and stays untouched; the token remains the generic `demo-token` (no name/symbol — its constructor is `(admin, hub_address)` only).
- No licensed KYC issuer. The token's issuer IS the attestor: identity/country is entered through the console UI and written to that token's own identity instance (the only manually-entered data, by design).
- No backend/server for the product path. All launch/console actions are client-side Freighter-signed. (The existing dev-only Vite middleware stays for the legacy demo only.)
- No heavy visual polish — that is SP3 (Direction-B "Constellation" reskin). SP2 delivers the functional flow on a clean, decomposed skeleton.

## Architecture

### One-time platform bootstrap (not per-user)

The multi-tenant model means the hub + shared modules are deployed **once** by the platform, then every user's launch is just a `launch(config)` call against that shared stack. A bootstrap script (mirroring `scripts/deploy-testnet.sh`) deploys to testnet:

- one `Hub` instance (platform admin = the bootstrap deployer key),
- one shared instance of each of the 7 `hub-module-*` contracts,
- uploads the `demo-token` and `identity-mock` wasm and records their hashes,
- calls `set_token_wasm`, `set_identity_wasm`, and `set_module_addr(kind, addr)` for all 7 modules.

It writes the resulting IDs to a new committed config file **`web/src/hub.testnet.json`** (shape below). This file is the frontend's only knowledge of the platform; it never holds a secret (the platform-admin key signs only the one-time bootstrap, never anything in the browser).

```json
{
  "network": "testnet",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "hub": "C...",
  "modules": {
    "denylist": "C...", "max_balance": "C...", "country_restrict": "C...",
    "max_holders": "C...", "lockup": "C...", "transfer_window": "C...",
    "max_investors": "C..."
  }
}
```

### Frontend stack (unchanged tooling)

React 18 + TypeScript + Vite, plain CSS (dark theme, extended with a small set of "Constellation" tokens). **New dependency:** `react-router-dom` for routing. No state library — `useReducer` for wizard state, `useState`/`localStorage` elsewhere. The existing `@stellar/stellar-sdk` v15 and `@stellar/freighter-api` stay.

### Contract-interaction layer (`stellar.ts` additions)

New exports, all reusing the proven build → `prepareTransaction` → Freighter-sign → `sendTransaction` → poll shape already in `stellar.ts` (`txTransfer`/`signSendPoll`). Compliance rejections surface at `prepareTransaction` (simulation) before any signature — the same mechanism the legacy demo uses for `explainDenial`.

- `launchConfigScVal(config)` — encodes the `LaunchConfig` struct as an `ScVal` map, built manually (the `proofScVal` manual `ScMap` pattern already in `stellar.ts`). Struct fields are emitted as an `ScMap` keyed by field-name symbols in the SDK's required (sorted) order: `admin, country_restrict, denylist, lockup, max_balance, max_holders, max_investors, transfer_window`. A golden round-trip test pins the encoding (see Testing).
- `launchToken(config, sign)` → returns `{ token, txHash }`. Calls `hub.launch(config)`; decodes the returned `LaunchResult.token` address from the tx result.
- Console wrappers over the hub's forwarders/reads (all wallet-signed writes, simulated reads): `mint`, `attestCountry(token, account, code)` (calls `set_country` on `hub.identity(token)`), `setInvestorCap`, `setMaxBalance`, `setMaxHolders`, `setLockup`, `pause`/`unpause`, `setWindow`, `addToDenylist`/`removeFromDenylist`, and reads `identity(token)`, `investorCap`/`investorCount`, `maxBalance`, `holders`, `isDenied`, `isPaused`, plus token `balance`/`totalSupply`.

### Routing & component decomposition

Introduce `react-router-dom`. `App.tsx` is decomposed from one 410-line component into:

- `routes`: `/` (Landing), `/launch` (Wizard), `/token/:id` (Console), `/zk` (the existing ZK + transfer demo, moved verbatim so it is not broken).
- shared `WalletContext` (connect/address/sign) lifted out of `App.tsx` so every route uses one wallet session.
- The legacy demo JSX (token stats, modules list, holders table, real-transfer card, ZK cards) moves into a `LegacyDemo` route component **unchanged in behavior** — its state slice and the dev `/api/*` middleware continue to work.

## Screens & flow

### 1. Landing (`/`)

Constellation hero + primary CTA "Launch your compliance token" → `/launch`. Secondary link to the ZK eligibility demo (`/zk`). Brief explainer of the 7 modules.

### 2. Launch Wizard (`/launch`) — 4 steps, `useReducer` state

- **Basics** — requires a connected Freighter wallet; `admin = connected address` (shown, not editable). If not connected, a connect prompt. Token is generic (no metadata field).
- **Compliance** — 7 controls, each toggling a `LaunchConfig` field, with inline config where the field is non-boolean:
  - denylist → bool
  - max_balance → i128 amount (0 = off)
  - country_restrict → multiselect of ISO-3166 numeric codes (empty = off)
  - max_holders → u32 (0 = off)
  - lockup → u64 seconds (0 = off)
  - transfer_window → bool
  - max_investors → u32 per-country cap (0 = off)
  - Inline hint when both `country_restrict` and `max_investors` are on: "these share one identity for this token."
- **Review** — human-readable summary of the exact `LaunchConfig` to be signed.
- **Launch** — one Freighter signature → progress (prepare → sign → send → poll) → success panel: token address (stellar.expert link), the launch tx hash, and "Open token console" → `/token/:id`. The launched token `{ id, config, admin, createdAt }` is appended to `localStorage["constella.tokens.<admin>"]`.

### 3. Token Console (`/token/:id`) — exercise & verify

Loads the token's config from `localStorage` (falls back to reading on-chain module registrations if absent). Sections, each only shown if the relevant module was selected:

- **Overview** — token address, admin, which modules are active, total supply, holder count.
- **Mint** — mint N tokens to an address (issuer-signed). This is how the issuer seeds holders to test.
- **Attest identity** (shown if country_restrict or max_investors) — enter an address + ISO country code → writes `set_country` on `hub.identity(token)`. Reads back the attested country. This is the sole manual-data entry point.
- **Manage** (forwarders) — set caps (max_balance/max_holders/max_investors), pause/unpause, set window, add/remove denylist — each issuer-signed.
- **Exercise** — attempt a transfer or mint between chosen addresses/amounts and **show the live result**: success, or the on-chain compliance rejection reason (reusing `explainDenial`). This is the "see the restriction actually work" surface. Includes read-outs (`investor_count(country)`, `is_denied`, `is_paused`, balances) that update after each action.

## Data flow

1. Bootstrap (once, offline script) → `hub.testnet.json` committed.
2. User connects Freighter → `WalletContext` holds `{ address, sign }`.
3. Wizard builds a `LaunchConfig` object → `launchConfigScVal` → `hub.launch` (one signature) → token address returned, persisted to `localStorage`.
4. Console reads that token's state via simulation and drives issuer actions via signed forwarder calls; every mutating call routes through `prepareTransaction` first, so a compliance violation is shown as a rejection reason rather than a failed signature.

## Error handling

- No wallet / wrong network → inline prompt (reuse `connectWallet`'s friendly errors; surface passphrase mismatch as "switch Freighter to Testnet").
- Launch simulation failure (e.g. platform not bootstrapped, module address unset) → show the simulation error verbatim in the Launch step; do not request a signature.
- Console actions that violate compliance → caught at `prepareTransaction`, rendered as the human reason (extend the existing `explainDenial` mapping to the hub's revert shapes). Distinguish "compliance rejected" (expected, informative) from "network/tx error" (retryable).
- `localStorage` empty on console load → attempt to reconstruct active modules from on-chain `modules_for(token, hook)`; if the token isn't found, show a "launch a token first" empty state.

## Testing

- **Unit (Node, like `verify-encoder.mjs`):** `launchConfigScVal` golden round-trip — encode a known `LaunchConfig`, assert the exact `ScVal` XDR base64 against a fixture generated from the Rust `LaunchConfig` type (or from a `nativeToScVal` reference), covering every field including empty/zero sentinels and a multi-country `country_restrict`.
- **Type/build gate:** `tsc --noEmit` + `vite build` clean; no regression to the legacy `/zk` flow (manual smoke: prove eligibility still works).
- **Live testnet proof (the real acceptance test, matching the contract-layer discipline):** run the bootstrap script; from the wizard, launch a token with `max_investors: 1` + `country_restrict:[US]` in one signature; in the console, attest two US holders, mint to the first (passes), attempt the second (console shows the per-country-cap rejection), then raise the cap and see it pass. Record the tx hashes.

## Build order (for the plan)

1. **Platform bootstrap** — script + `hub.testnet.json` (deploy the shared stack to testnet, commit the config).
2. **stellar.ts launch layer** — `launchConfigScVal` (+ golden test) and `launchToken`.
3. **Routing + decomposition** — add `react-router-dom`, `WalletContext`, move legacy demo to `/zk` unchanged, stub Landing/Wizard/Console routes.
4. **Launch Wizard** — 4-step `useReducer` flow → real one-signature launch → localStorage persistence.
5. **Token Console** — overview/mint/attest/manage/exercise, wired to the new stellar.ts wrappers.
6. **Live testnet verification** — bootstrap + end-to-end launch + console rejection, record hashes.

Each step is independently testable (a working, buildable frontend after each).

---

## SP2 — live testnet evidence (platform stack)

The platform bootstrap (Task 1) deployed the shared Hub + all 7 modules to testnet and committed their IDs to `web/src/hub.testnet.json` (hub `CBKJR7KRQWWGL7CGCEOYMRECGGPH5O3RUKOL37GWWKJ5IJQ7HP5BAQCG`). End-to-end verification against that exact committed stack — the same hub the browser wizard calls:

- **One-signature launch** (`country_restrict:[US 840] + max_investors:1`): tx [`b5537793…`](https://stellar.expert/explorer/testnet/tx/b55377934aa7e07823b57da3f26317bf8dbeb2d537b68125c0f72b289796a941) → token `CAJPOSVCZPBBILJRP3JZE54SVAAZDAZNKMCXVTBUOTOCLQ3JT4SETSAD`, `investor_cap = 1`.
- The hub deployed a per-token identity (`CBRNAIBE…RQVV`); the issuer attested two accounts as US(840) on it.
- Mint to the 1st US holder passed (`investor_count(US) = 1`); mint to the 2nd **reverted** (per-country cap 1); `set_investor_cap(token, 2)` via the forwarder then let the 2nd through (`investor_count(US) = 2`). Restrictions enforce live, driven by the same forwarders the console UI calls.

Frontend gates: `npm run verify:launch` (LaunchConfig ScVal encoder golden) ✅, `tsc --noEmit` clean, `vite build` clean. The `launchConfigScVal` encoding is byte-correct against the Rust `LaunchConfig` struct (reviewed field-by-field). Browser Freighter click-through (wizard → console) is the final human verification step, run against this same bootstrapped hub.
