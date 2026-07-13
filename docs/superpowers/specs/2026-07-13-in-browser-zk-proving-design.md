# In-browser client-side ZK proving — design

**Date:** 2026-07-13 · **Status:** approved (pre-implementation)

## Goal

Move Groth16 proof **generation** into the browser so a holder proves their country is
in the allowed set **without any server or admin ever seeing the country** — and make
the eligibility flow work on a publicly-hosted static site (no local `stellar` CLI, no
admin key). This is a functional, demo-grade slice; production hardening (trusted-setup
ceremony, real KYC binding, key management) remains **Instaward #2**.

### Non-goals (explicit)

- ZK-gated token mint/transfer end-to-end on the public site (mint is an issuer action).
- Production trusted setup / KYC attestation binding.
- Changing the cleartext demo cards (they stay dev-only middleware).

## Current state (what exists)

- Circuit `zk/circuits/country_eligibility.circom` — Poseidon(2) commitment + set-membership
  over BLS12-381, N=2 allowed, `allowed[]` public, `country`/`secret` private, `commitment`
  a public output.
- Proving is **offline + fixed**: `zk/build.sh` (snarkjs) produces `zk/data/{proof,public,
  verification_key}.json`; `tools/zk-encode` (Rust/arkworks) encodes them to the Soroban
  byte format; the web demo submits that one fixed proof via a **dev-only Vite middleware**
  (`/api/zk-prove`) using the admin `deployer` CLI key.
- Contract `crates/module-identity-zk`:
  - `register_commitment(account, commitment)` — **admin-gated** (issuer attests).
  - `prove_eligibility(account, commitment, proof)` — **already permissionless**; checks the
    registered commitment, verifies Groth16 against on-chain VK + `allowed`, sets `Eligible`.
  - On-chain VK is set once via `set_policy` and matches `zk/build/country_eligibility_final.zkey`.

## Design

### 1. Contract change — `crates/module-identity-zk`

Add a permissionless-but-authenticated self-registration path so a holder can register
their own commitment with just their wallet:

```rust
/// Demo self-attestation: the holder registers their own commitment (wallet-authed).
/// Production keeps issuer attestation via `register_commitment`.
pub fn register_self(env: Env, account: Address, commitment: U256) {
    account.require_auth();
    env.storage().persistent().set(&DataKey::Commitment(account), &commitment);
}
```

- `register_commitment` (admin) is **unchanged** — the production path stays.
- `prove_eligibility` is **unchanged** — already permissionless; the proof is the authorization.
- Redeploy identity-zk (new wasm). VK/policy unchanged, so `set_policy` runs as today.

### 2. Web proving core — `web/src/zk/`

- **`prove.ts`** — `generateProof(country: number, secret: bigint)`: calls
  `snarkjs.groth16.fullProve({ country, secret, allowed: [840, 276] }, wasmUrl, zkeyUrl)`.
  Returns `{ proof, publicSignals }`. `publicSignals[0]` **is** the commitment (a public
  circuit output) — no separate Poseidon computation needed. A disallowed `country` makes
  the witness unsatisfiable and `fullProve` throws — this is the denial signal.
- **`encode.ts`** — a TypeScript port of `tools/zk-encode`. `le48(x: bigint): Uint8Array`
  (48-byte little-endian) plus `g1(x,y) -> 96B`, `g2(x0,x1,y0,y1) -> 192B`, `fr(dec) -> 32B
  big-endian`, concatenated in the **same field order as the Rust encoder**. Produces the
  `{ a, b, c }` byte blobs `prove_eligibility` expects. Correctness is guaranteed by a
  golden-reference test (§4).
- **`submit.ts`** — with `@stellar/stellar-sdk`: build and submit two Soroban transactions,
  each signed by the connected wallet via Freighter:
  1. `register_self(account, commitment)`
  2. `prove_eligibility(account, commitment, proof)`
  Returns the two tx hashes. (Two txs = two signatures; a combined `register_and_prove`
  contract fn is a possible future optimization but out of scope here.)

### 3. Static artifacts — `web/public/zk/`

Copy the **exact** artifacts whose VK is deployed on-chain:
- `country_eligibility.wasm` (~1.7 MB, witness calculator)
- `country_eligibility_final.zkey` (~364 KB, proving key)

Committed under `web/public/zk/` (outside the `zk/build/` gitignore). Serving the same zkey
that produced the on-chain VK guarantees browser-generated proofs verify on-chain.

### 4. UI — the ZK-eligibility card in `web/src/App.tsx`

- A **country dropdown**: allowed (🇺🇸 840, 🇩🇪 276) and disallowed (🇹🇷 792, 🇫🇷 250) options.
- `secret`: 64-bit random via `crypto.getRandomValues` per attempt.
- **"Prove eligibility (zero-knowledge)"** → `generateProof` (spinner "Generating proof in
  your browser…"):
  - success → `encode` → `submit` (two Freighter prompts) → show `is_verified = true`, the
    "country stayed private" line, and both tx explorer links.
  - `fullProve` throws → show a privacy-preserving denial: "Not eligible — and the app never
    learned or transmitted your country."
- This card **no longer** calls `/api/zk-prove`. The dev-only middleware cards are untouched.

## Testing

- **Contract (TDD, `module-identity-zk/src/test.rs`):**
  1. A non-admin account calls `register_self`, then `prove_eligibility` with a valid proof →
     `is_verified(account) == true`.
  2. `register_self` for an account without that account's auth → panics.
- **Encoder golden test (`web/scripts/verify-encoder.mjs`):** run the TS encoder on
  `zk/data/proof.json` + `verification_key.json`; assert its hex output **equals** the Rust
  `tools/zk-encode` output (captured as a golden fixture). Byte-for-byte or fail.
- **E2E (manual, after redeploy):** in the browser against redeployed testnet contracts —
  allowed country → `is_verified` flips true with two real tx hashes; disallowed country →
  proof generation fails, no tx, denial shown.

## Risks & fallback

- **Primary risk — snarkjs under Vite:** bundling snarkjs (Node builtins, web-worker/wasm
  loading) can need Vite config tweaks. Time-box ~half a day. Fallback: run proving in a Web
  Worker; if still blocked, ship the snarkjs prebuilt browser bundle as a static script.
- **Encoder byte-format risk:** fully mitigated by the golden-reference test before any
  on-chain submission.
- **Artifact/VK drift:** never regenerate the zkey independently — always serve the one whose
  VK is on-chain. Documented at the copy step.

## Deploy / rollout

1. Land the contract change (TDD) → redeploy identity-zk via `scripts/deploy-testnet.sh`
   (fresh deploy picks up the new wasm; `set_policy` unchanged).
2. Copy artifacts to `web/public/zk/`; wire the web proving core + UI card.
3. Publish the static site (existing web build) — the ZK-eligibility card works with no admin.
