# Private (ZK) Eligibility for Launched Tokens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Wire the existing ZK stack into the multi-tenant hub so an issuer can launch a token whose country eligibility is proven privately (browser Groth16 proof) instead of attested in cleartext, and expose it in the launch wizard + token console. Remove the standalone `/zk` demo.

**Architecture:** A per-token `module-identity-zk` (deployed at launch, policy = the token's ≤2 allowed countries) gates transfers through a new token-keyed `hub-module-zk-eligibility` that checks `is_verified`. Holders prove eligibility in the browser. `VerificationKey`/`Proof` move to `module-interface` so the hub can carry the VK without depending on a `#[contract]` crate.

**Tech Stack:** Rust / soroban-sdk 26; React 18 / TS / Vite; snarkjs.

## Global Constraints

- English only (code, comments, commits, docs). Contracts build to `wasm32v1-none`; `cargo test --workspace` + `cargo clippy` clean (no bool-literal `assert_eq!`). Frontend: `tsc --noEmit` + `vite build` clean.
- Per-token isolation; module mutators/post-events `require_hub` first; hub forwarders gate `TokenAdmin(token).require_auth()`. Guard tests pinned to `#[should_panic(expected = "Error(Auth, InvalidAction)")]`.
- Hub depends only on `module-interface` (`#[contractclient]` traits + shared `#[contracttype]`), never on a `#[contract]` module/token/verifier crate.
- Circuit is fixed to **2 allowed countries**; ZK tokens use `country_restrict` (≤2 entries) as the allowed set. ZK ⊥ `max_investors` on one token.
- Do NOT change any `#[contracttype]` field layout when moving types (on-chain ABI must be byte-identical).
- Work on branch `sp2/launch-wizard` (the frontend branch); contracts land alongside.

---

### Task 1: Move `VerificationKey` + `Proof` into `module-interface`

**Files:** Modify `crates/module-interface/src/lib.rs`, `crates/zk-verifier/src/lib.rs`, `crates/module-identity-zk/src/lib.rs`, `crates/zk-verifier/Cargo.toml`

**Interfaces:** Produces `constella_module_interface::{VerificationKey, Proof}` (identical layout to today's `zk-verifier` definitions).

- [ ] **Step 1: Read the current definitions** — `crates/zk-verifier/src/lib.rs` lines around 25–42. Copy the exact `#[contracttype] pub struct VerificationKey { … }` and `pub struct Proof { … }` (including every field, type, and order — the BLS12-381 `G1Affine`/`G2Affine`/`Fr`/`Vec` fields). Note the exact `use` items they need (`soroban_sdk::crypto::bls12_381::*`, `Vec`, etc.).

- [ ] **Step 2: Add them to `module-interface`** — paste both structs verbatim into `crates/module-interface/src/lib.rs` with the required imports. Keep `#[contracttype]`. Do not rename fields or reorder.

- [ ] **Step 3: Re-point `zk-verifier`** — in `crates/zk-verifier/src/lib.rs`, delete the local `VerificationKey`/`Proof` definitions and instead `use constella_module_interface::{VerificationKey, Proof};`. Add `constella-module-interface = { path = "../module-interface" }` to `crates/zk-verifier/Cargo.toml` `[dependencies]`.

- [ ] **Step 4: Re-point `module-identity-zk`** — change `use constella_zk_verifier::{Groth16VerifierClient, Proof, VerificationKey};` to `use constella_zk_verifier::Groth16VerifierClient;` + `use constella_module_interface::{Proof, VerificationKey};` (module-identity-zk already depends on module-interface? if not, add the path dep).

- [ ] **Step 5: Build + test + commit**
```bash
stellar contract build >/dev/null 2>&1 && echo "wasm ok"
cargo test -p constella-zk-verifier -p constella-module-identity-zk 2>&1 | grep -E "test result|error" | head
cargo clippy -p constella-module-interface -p constella-zk-verifier -p constella-module-identity-zk 2>&1 | grep -E "warning|error" | head
git add crates/module-interface crates/zk-verifier crates/module-identity-zk
git commit -m "refactor(zk): move VerificationKey + Proof into module-interface (shared, no #[contract] dep)"
```
Expected: wasm builds, existing ZK tests still pass (ABI unchanged), clippy clean.

---

### Task 2: `hub-module-zk-eligibility` — token-keyed gate on `is_verified` (TDD)

**Files:** Create `crates/hub-module-zk-eligibility/{Cargo.toml, src/lib.rs, src/test.rs}`

**Interfaces:** `ZkEligibilityHubModule` — `__constructor(hub)`, `configure(token, identity)` (hub-authed), `can_create`/`can_transfer` (gate on recipient `is_verified`), no-op post-events, `identity(token)`.

- [ ] **Step 1: Cargo.toml** — name `constella-hub-module-zk-eligibility`; deps `soroban-sdk`, `constella-module-interface`; dev-deps testutils + `constella-identity-mock` (its `is_verified` is settable via `set_verified` — verify the mock exposes a way to set eligibility; if not, use `constella-module-identity-zk` is heavier — prefer identity-mock's `set_verified` if present, else add a tiny `#[cfg(test)]` mock). Check `crates/identity-mock/src/lib.rs` for `set_verified`/`is_verified`.

- [ ] **Step 2: Failing tests** — `src/test.rs`. Model on `crates/hub-module-country-restrict/src/test.rs`. Two tokens, each with its own identity; set one account verified on token A's identity only; assert `can_create` true for verified, false for unverified, and isolation across tokens. Pin the two guards (`configure` + a post-event) to `#[should_panic(expected = "Error(Auth, InvalidAction)")]`.

- [ ] **Step 3: Run RED** — `cargo test -p constella-hub-module-zk-eligibility 2>&1 | tail -8`.

- [ ] **Step 4: Implement** — `src/lib.rs` (mirrors `crates/module-zk-eligibility` + token-keying + hub-auth):
```rust
#![no_std]
//! Multi-tenant ZK-eligibility module: gates on the recipient's ZK eligibility flag
//! (`is_verified`) read from the token's per-token ZK identity — never a cleartext
//! country. Stateless gate; all config keyed by token.
use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey { Hub, Identity(Address) }

#[contract]
pub struct ZkEligibilityHubModule;

#[contractimpl]
impl ZkEligibilityHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn configure(env: Env, token: Address, identity: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Identity(token), &identity);
    }
    pub fn identity(env: Env, token: Address) -> Address {
        env.storage().persistent().get(&DataKey::Identity(token)).unwrap()
    }
    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }
    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }
    pub fn transferred(_e: Env, _f: Address, _t: Address, _a: i128, _tok: Address) {}
    pub fn created(_e: Env, _t: Address, _a: i128, _tok: Address) {}
    pub fn destroyed(_e: Env, _f: Address, _a: i128, _tok: Address) {}
}

impl ZkEligibilityHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn eligible(env: &Env, token: &Address, who: &Address) -> bool {
        let id: Address = env.storage().persistent().get(&DataKey::Identity(token.clone())).unwrap();
        IdentityClient::new(env, &id).is_verified(who)
    }
}
```

- [ ] **Step 5: GREEN + clippy + commit**
```bash
cargo test -p constella-hub-module-zk-eligibility 2>&1 | tail -6
cargo clippy -p constella-hub-module-zk-eligibility --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub-module-zk-eligibility && git commit -m "feat(hub): token-keyed Zk-eligibility module (gates on is_verified, country stays private)"
```

---

### Task 3: `module-interface` — `ZkEligibilityClient` + `IdentityZkAdminClient` traits

**Files:** Modify `crates/module-interface/src/lib.rs`

- [ ] **Step 1: Append the traits** (beside the others; `VerificationKey` is now local to this crate from Task 1):
```rust
/// Config surface of the multi-tenant ZkEligibility module, called by the hub. Token-keyed.
#[contractclient(name = "ZkEligibilityClient")]
pub trait ZkEligibilityAdmin {
    fn configure(env: Env, token: Address, identity: Address);
    fn identity(env: Env, token: Address) -> Address;
}

/// Policy surface of the per-token ZK identity (`module-identity-zk`), called by the hub at launch.
#[contractclient(name = "IdentityZkAdminClient")]
pub trait IdentityZkAdmin {
    fn set_policy(env: Env, vk: VerificationKey, allowed: Vec<u32>);
    fn is_verified(env: Env, account: Address) -> bool;
}
```

- [ ] **Step 2: Build + commit**
```bash
cargo build -p constella-module-interface 2>&1 | tail -1
cargo clippy -p constella-module-interface --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/module-interface/src/lib.rs && git commit -m "feat(interface): ZkEligibilityClient + IdentityZkAdminClient traits"
```

---

### Task 4: Hub wiring — platform config + `zk_eligibility` launch branch (TDD)

**Files:** Modify `crates/hub/src/lib.rs`, `crates/hub/src/test.rs`, `crates/hub/README.md`

**Interfaces:** platform config `set_verifier(addr)`, `set_zk_identity_wasm(hash)`, `set_zk_vk(vk)`; `LaunchConfig.zk_eligibility: bool`; read `is_verified(token, account) -> bool`.

- [ ] **Step 1: Failing e2e test** — append to `crates/hub/src/test.rs`. Add `contractimport!` for `constella_zk_verifier`, `constella_module_identity_zk`, and the new `constella_hub_module_zk_eligibility` wasm. Use `tools/zk-encode` output committed as a fixture, OR import the golden VK/proof/commitment the existing ZK tests already use (check `crates/module-identity-zk/src/test.rs` for how it builds a VK + proof in-test — reuse that exact construction). The test: bootstrap the hub with `set_verifier` + `set_zk_identity_wasm` + `set_zk_vk` (VK from the fixture) + `set_module_addr("zk_eligibility", …)`; `launch({…, country_restrict:[840,276], zk_eligibility:true})`; read `hub.identity(token)`; on that ZK identity `register_self` + `prove_eligibility` for an account with the fixture proof; assert the token's `mint`/create to that account passes and to an unproven account is rejected.
  - If constructing a valid proof in a hub unit test is impractical, split: keep the module-level `is_verified` gating proven in Task 2 (with a mock identity), and make THIS test assert only the wiring — that `launch` with `zk_eligibility` deploys a ZK identity (`hub.identity(token)` resolves), registers the module on both pre-check hooks, and that an account not yet verified is denied (mint reverts). Prove the full proof path live on testnet in Task 7. Note the chosen split in the test comment.

- [ ] **Step 2: Run RED** — `stellar contract build >/dev/null && cargo test -p constella-hub zk 2>&1 | tail -12`.

- [ ] **Step 3: Implement** — `crates/hub/src/lib.rs`:
  - Import `use constella_module_interface::{…, ZkEligibilityClient, IdentityZkAdminClient, VerificationKey};`.
  - `DataKey`: add `Verifier, ZkIdentityWasm, ZkVk`.
  - Add `LaunchConfig.zk_eligibility: bool` (last field).
  - Platform config setters (mirror `set_identity_wasm`):
```rust
    pub fn set_verifier(env: Env, verifier: Address) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::Verifier, &verifier);
    }
    pub fn set_zk_identity_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::ZkIdentityWasm, &hash);
    }
    pub fn set_zk_vk(env: Env, vk: VerificationKey) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::ZkVk, &vk);
    }
    pub fn is_verified(env: Env, token: Address, account: Address) -> bool {
        let id: Address = env.storage().persistent().get(&DataKey::Identity(token)).unwrap();
        IdentityZkAdminClient::new(&env, &id).is_verified(&account)
    }
```
  - In `launch`, in the identity-dependent region, branch on `zk_eligibility` FIRST (it owns the token's identity when set):
```rust
        if config.zk_eligibility {
            // Private country eligibility: deploy a per-token ZK identity, set its policy to the
            // chosen allowed set, and gate on is_verified. Skips cleartext country_restrict/max_investors.
            let zk_hash: BytesN<32> = env.storage().instance().get(&DataKey::ZkIdentityWasm).unwrap();
            let verifier: Address = env.storage().instance().get(&DataKey::Verifier).unwrap();
            let vk: VerificationKey = env.storage().instance().get(&DataKey::ZkVk).unwrap();
            let identity = Self::deploy(&env, &zk_hash, (config.admin.clone(), verifier));
            IdentityZkAdminClient::new(&env, &identity).set_policy(&vk, &config.country_restrict);
            env.storage().persistent().set(&DataKey::Identity(token.clone()), &identity);
            let m = Self::module_addr(&env, "zk_eligibility");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            ZkEligibilityClient::new(&env, &m).configure(&token, &identity);
        }
```
  Then guard the existing cleartext identity/hoist blocks so a ZK token doesn't ALSO deploy a cleartext identity or register country_restrict/max_investors. Concretely, change the hoisted-identity condition and the two identity blocks to `… && !config.zk_eligibility`:
  - hoisted deploy: `if !config.zk_eligibility && (!config.country_restrict.is_empty() || config.max_investors > 0) { … }`
  - country_restrict block: `if !config.zk_eligibility && !config.country_restrict.is_empty() { … }`
  - max_investors block: `if !config.zk_eligibility && config.max_investors > 0 { … }`

- [ ] **Step 4: GREEN + migrate literals** — `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -8`. Add `zk_eligibility: false` to EVERY existing `LaunchConfig` literal in `test.rs` (grep count of `transfer_window:` should equal count of `zk_eligibility:` after).

- [ ] **Step 5: fmt/clippy/README/commit** — document the ZK branch + the 3 platform setters + the mutual exclusion in `crates/hub/README.md`. Then:
```bash
cargo fmt -p constella-hub && cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub && git commit -m "feat(hub): wire Zk-eligibility launch path (per-token ZK identity + policy) + platform VK/verifier config"
```

---

### Task 5: Bootstrap the shared ZK stack + `hub.testnet.json`

**Files:** Modify `scripts/bootstrap-hub-testnet.sh`, `web/src/hub.testnet.json`

- [ ] **Step 1: Extend the bootstrap** — after the module deploys, add (following the ZK section of `scripts/deploy-testnet.sh`, lines ~125–135, and the existing indexed-array/`LC_ALL=C` style of this script):
```bash
echo "▸ Deploying shared ZK stack…"
ZKID_HASH=$(stellar contract upload --wasm "$W/constella_module_identity_zk.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)
VERIFIER=$(stellar contract deploy --wasm "$W/constella_zk_verifier.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)
ELIG=$(stellar contract deploy --wasm "$W/constella_hub_module_zk_eligibility.wasm" --source deployer --network "$NET" -- --hub "$HUB" 2>/dev/null | tail -1)
ZKARGS=$(cargo run --manifest-path tools/zk-encode/Cargo.toml --quiet 2>/dev/null)
VKJSON=$(printf '%s' "$ZKARGS" | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{const j=JSON.parse(s);console.log(JSON.stringify(j.vk))})")
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_verifier --verifier "$VERIFIER" >/dev/null
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_zk_identity_wasm --hash "$ZKID_HASH" >/dev/null
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_zk_vk --vk "$VKJSON" >/dev/null
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_module_addr --kind zk_eligibility --addr "$ELIG" >/dev/null
```
  Add `"zk_eligibility": "$ELIG"` to the emitted `modules` block and top-level `"verifier": "$VERIFIER"`.

- [ ] **Step 2: Run + verify + commit** — `bash scripts/bootstrap-hub-testnet.sh`; confirm the JSON now has 8 module addresses + a `verifier`; `git add scripts/bootstrap-hub-testnet.sh web/src/hub.testnet.json && git commit -m "feat(sp3): bootstrap shared ZK stack (verifier + zk-identity wasm + VK) into the hub"`. (Redeploying the platform is fine — records fresh IDs.)

---

### Task 6: Frontend — ZK launch option + console prove flow; remove the demo

**Files:** Modify `web/src/zk/prove.ts`, `web/src/hub.ts`, `web/scripts/verify-launch-encoder.mjs`, `web/src/routes/LaunchWizard.tsx`, `web/src/routes/TokenConsole.tsx`, `web/src/App.tsx`, `web/src/routes/Docs.tsx`, `web/src/routes/Landing.tsx`; Delete `web/src/routes/LegacyDemo.tsx`

- [ ] **Step 1: Parametrize the prover** — `web/src/zk/prove.ts`: change `generateProof(country, secret)` to `generateProof(country, secret, allowed: string[])` and use the passed `allowed` in `input`. Keep the `IneligibleError` mapping. (The single existing caller is the demo, which is being removed; the new caller is `hub.ts`.)

- [ ] **Step 2: hub.ts — config field + prove flow** —
  - `LaunchConfig` type: add `zk_eligibility: boolean`; `blankConfig`: `zk_eligibility: false`.
  - `launchConfigScVal`: add `e('zk_eligibility', xdr.ScVal.scvBool(cfg.zk_eligibility))` in sorted position (LAST, after `transfer_window`).
  - Add reads/flow using the existing `signSendPoll`/`server`/`addr`:
```ts
export async function readZkAllowed(token: string): Promise<number[]> {
  const identity = await readIdentity(token);
  const s = await sim(identity, 'allowed', []);
  return rpc.Api.isSimulationError(s) ? [] : (toNative(s.result!.retval) as number[]);
}
export async function readIsVerified(token: string, account: string): Promise<boolean> {
  const s = await sim(HUB, 'is_verified', [scAddr(token), scAddr(account)]);
  return rpc.Api.isSimulationError(s) ? false : toNative(s.result!.retval) === true;
}
// Holder self-prove: read the token's ZK identity + allowed set, prove in-browser, submit.
export async function proveEligibility(
  token: string, account: string, country: number, sign: SignFn,
  onStep?: (phase: 'register' | 'prove') => void,
): Promise<{ registerHash: string; proveHash: string }> {
  const identity = await readIdentity(token);
  const allowed = (await readZkAllowed(token)).map(String);
  const secret = BigInt('0x' + [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, '0')).join(''));
  const { proof, commitment } = await generateProof(country, secret, allowed); // import from ./zk/prove
  const bytes = encodeProof(proof); // import from ./zk/encode
  // register_self then prove_eligibility on the per-token ZK identity (two signed txs)
  onStep?.('register');
  const acc1 = await server.getAccount(account);
  const regHash = await signSendPoll(buildFrom(acc1, identity, 'register_self', [scAddr(account), u256(commitment)]), sign, 'register_self');
  onStep?.('prove');
  const acc2 = await server.getAccount(account);
  const proveHash = await signSendPoll(buildFrom(acc2, identity, 'prove_eligibility', [scAddr(account), u256(commitment), proofScVal(bytes)]), sign, 'prove_eligibility');
  return { registerHash: regHash, proveHash };
}
```
  Reuse the `u256` + `proofScVal` construction already used by `submitZkEligibility` in `stellar.ts` — either export them from `stellar.ts` and import here, or replicate the tiny `proofScVal` (symbol-keyed ScMap `{a,b,c}`) and `u256` in `hub.ts`. Import `generateProof`/`encodeProof` from `./zk/prove` + `./zk/encode`. Add any missing SDK imports (`toNative` alias already added in Task 5's console work).

- [ ] **Step 3: Golden encoder** — `web/scripts/verify-launch-encoder.mjs`: add `zk_eligibility: false` to the fixture config and `e('zk_eligibility', xdr.ScVal.scvBool(cfg.zk_eligibility))` (last) to the inline encoder. `cd web && npm run verify:launch` → ✅.

- [ ] **Step 4: Wizard — "Private (ZK)" toggle** — in the Country-restrict module row (`LaunchWizard.tsx`), add a small switch: "Private (prove with ZK)". When on: `set('zk_eligibility', true)`, cap `country_restrict` to 2 entries (ignore/deny a 3rd with a hint), and force `max_investors` to 0 (disable its input with a note "unavailable with private eligibility"). When off: `set('zk_eligibility', false)`. Show a `zk` star in the constellation when on (add to `activeMods`: push `'zk'` when `cfg.zk_eligibility`, and drop `'country'`/`'investors'`). Review step shows "Country eligibility: private (ZK) · US, DE".

- [ ] **Step 5: Console — Prove eligibility panel** — in `TokenConsole.tsx`, when `cfg.zk_eligibility`, replace the "Attest identity" panel with a "Prove eligibility (ZK)" panel: a country `<select>` (the connected wallet's private country), a "Prove eligibility" button that runs `proveEligibility(id, address, country, sign, onStep)` driving the **4-step pipeline** (port the `.zk-pipe` markup + `zkStep` state from the old demo), and a live `is_verified` status (`readIsVerified(id, address)`). Minting to a proven holder passes; to an unproven one the mint is rejected (existing Mint panel already surfaces it). Keep all non-ZK panels unchanged.

- [ ] **Step 6: Remove the demo** — delete `web/src/routes/LegacyDemo.tsx`; remove its `<Route path="/zk">` + import from `App.tsx`; remove the "Privacy/ZK demo" links from `App.tsx` nav, `Docs.tsx` (`#privacy` section link to `/zk`), and `Landing.tsx` (the ZK feature card can stay as a documented capability, but drop any `/zk` link). Update `Docs.tsx` ZK section to describe the in-product flow ("choose Private (ZK) at launch; holders prove in the console") instead of linking a demo. Keep `web/src/zk/prove.ts` + `encode.ts` (used by `hub.ts`).

- [ ] **Step 7: Typecheck, build, commit**
```bash
cd web && npm run verify:launch && npx tsc --noEmit && npm run build
git add -A web/src web/scripts && git commit -m "feat(sp3): private ZK eligibility in the launch wizard + token console; remove standalone demo"
```

---

### Task 7: Live testnet verification + evidence

- [ ] **Step 1: Workspace gate** — `stellar contract build >/dev/null && cargo test --workspace 2>&1 | grep -E "FAIL|^error" || echo green`; `cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1`.

- [ ] **Step 2: Testnet (controller)** — with the bootstrapped hub (Task 5): a funded issuer `launch`es a token with `country_restrict:[840,276], zk_eligibility:true`. Read `hub.identity(token)` (the ZK identity). For an eligible account: `register_self` + `prove_eligibility` with the golden proof from `tools/zk-encode` (whose country ∈ {840,276}); confirm `hub.is_verified(token, account)` → true and a `mint` to it passes. For an un-proven account: `mint` reverts (not eligible). Record the launch tx + the enforced revert. Append evidence to `crates/hub/README.md` and the spec.

- [ ] **Step 3: Final gate** — `cd web && npm run verify:launch && npx tsc --noEmit && npm run build`. Commit evidence. Feature complete: launched tokens can enforce **private** country eligibility, proven in the browser, with the country never revealed on-chain.
