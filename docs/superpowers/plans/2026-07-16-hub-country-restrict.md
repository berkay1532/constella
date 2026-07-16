# Multi-tenant Hub — add CountryRestrict (per-token identity) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first **identity-dependent** module (CountryRestrict) to the multi-tenant hub, validating the per-token identity mechanic (the last unproven piece): the hub deploys an identity provider per token, and the module reads the recipient's attested country from *that* token's identity.

**Architecture:** At `launch`, when an identity-dependent module is selected, the hub deploys an `identity-mock` instance for the token (admin = the issuer, reused unchanged via `deploy_v2`), stores `Identity(token)`, and configures the shared CountryRestrict module with that identity + the allow-list. The module keys `Identity(token)` + `Allowed(token)`, and on a pre-check reads `country_of(to)` from the token's own identity. The issuer attests directly on their per-token identity contract (they are its admin); the hub exposes `identity(token)`.

**Tech Stack:** Rust / soroban-sdk 26, stellar CLI (testnet).

## Global Constraints

- English; builds to `wasm32v1-none`; `cargo test --workspace` + `cargo clippy` green. TDD (RED before GREEN).
- **Per-token isolation:** `Identity(token)` and `Allowed(token)` both token-keyed; two tokens with different allow-lists/identities never affect each other. Every test uses two tokens.
- The hub must NOT depend on any `#[contract]` crate; calls modules via `#[contractclient]` traits (`ModuleClient`, a new `CountryRestrictClient`). `identity-mock` is deployed by wasm hash (`deploy_v2`), reused UNCHANGED.
- Module mutators (`configure`/`set_allowed`) require the hub's auth (`hub.require_auth()`). Pre-checks are token-keyed reads. Guard tests pinned to `#[should_panic(expected = "Error(Auth, InvalidAction)")]`.
- The v2 module reads `country_of` via the existing `IdentityClient` from `module-interface` (so it depends on `module-interface`).
- Hub tests `contractimport!` token + module + identity wasm → run `stellar contract build` before `cargo test -p constella-hub`.

---

### Task 1: `hub-module-country-restrict` — token-keyed identity-dependent module (TDD)

**Files:**
- Create: `crates/hub-module-country-restrict/Cargo.toml`, `src/lib.rs`, `src/test.rs`

**Interfaces:**
- Produces: `CountryRestrictHubModule` with `__constructor(hub: Address)`; `configure(token, identity, allowed: Vec<u32>)` + `set_allowed(token, allowed)` (hub-authed); reads `identity(token) -> Address`, `allowed(token) -> Vec<u32>`; hooks `can_transfer(from,to,amount,token)->bool` / `can_create(to,amount,token)->bool` (deny unless `country_of(to)` from `Identity(token)` is in `Allowed(token)`); no-op post-events.

- [ ] **Step 1: Cargo.toml**
```toml
[package]
name = "constella-hub-module-country-restrict"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
publish = false
[lib]
crate-type = ["lib", "cdylib"]
doctest = false
[dependencies]
soroban-sdk = { workspace = true }
constella-module-interface = { path = "../module-interface" }
[dev-dependencies]
soroban-sdk = { workspace = true, features = ["testutils"] }
constella-identity-mock = { path = "../identity-mock" }
```

- [ ] **Step 2: Write the failing two-token isolation + guard tests** — `crates/hub-module-country-restrict/src/test.rs`
```rust
#![cfg(test)]
use crate::{CountryRestrictHubModule, CountryRestrictHubModuleClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

fn setup(env: &Env) -> (CountryRestrictHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(CountryRestrictHubModule, (hub.clone(),));
    (CountryRestrictHubModuleClient::new(env, &id), hub)
}

#[test]
fn eligibility_isolated_per_token_and_identity() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);

    // Each token has its own identity provider + allow-list.
    let id_a = env.register(IdentityMock, (admin.clone(),));
    let id_b = env.register(IdentityMock, (admin.clone(),));
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.configure(&ta, &id_a, &vec![&env, 840u32]); // token A allows US
    m.configure(&tb, &id_b, &vec![&env, 276u32]); // token B allows DE

    let alice = Address::generate(&env);
    IdentityMockClient::new(&env, &id_a).set_country(&alice, &840); // US on A's identity
    IdentityMockClient::new(&env, &id_b).set_country(&alice, &792); // TR on B's identity

    assert!(m.can_create(&alice, &1, &ta));  // US ∈ {US} on token A
    assert!(!m.can_create(&alice, &1, &tb)); // TR ∉ {DE} on token B — isolated
    // unattested recipient is denied
    let bob = Address::generate(&env);
    assert!(!m.can_create(&bob, &1, &ta));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn configure_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env); // no mock_all_auths
    m.configure(&Address::generate(&env), &Address::generate(&env), &vec![&env, 840u32]);
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p constella-hub-module-country-restrict 2>&1 | tail -12`
Expected: FAIL to compile — the type doesn't exist.

- [ ] **Step 4: Implement** — `crates/hub-module-country-restrict/src/lib.rs`
```rust
#![no_std]
//! Multi-tenant CountryRestrict module: only allows holders whose attested country
//! (from that token's own identity provider) is in the token's allow-list. One shared
//! instance serves every token; identity + allow-list keyed by token. Reads only the
//! identity boundary (no balance mirror), so post-events are no-ops.

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Identity(Address), // token -> identity provider
    Allowed(Address),  // token -> allowed country codes
}

#[contract]
pub struct CountryRestrictHubModule;

#[contractimpl]
impl CountryRestrictHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }

    pub fn configure(env: Env, token: Address, identity: Address, allowed: Vec<u32>) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Identity(token.clone()), &identity);
        env.storage().persistent().set(&DataKey::Allowed(token), &allowed);
    }

    pub fn set_allowed(env: Env, token: Address, allowed: Vec<u32>) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Allowed(token), &allowed);
    }

    pub fn identity(env: Env, token: Address) -> Address {
        env.storage().persistent().get(&DataKey::Identity(token)).unwrap()
    }

    pub fn allowed(env: Env, token: Address) -> Vec<u32> {
        env.storage().persistent().get(&DataKey::Allowed(token)).unwrap()
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl CountryRestrictHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn eligible(env: &Env, token: &Address, who: &Address) -> bool {
        let identity: Address = env.storage().persistent().get(&DataKey::Identity(token.clone())).unwrap();
        let allowed: Vec<u32> = env.storage().persistent().get(&DataKey::Allowed(token.clone())).unwrap();
        match IdentityClient::new(env, &identity).country_of(who) {
            Some(code) => allowed.iter().any(|c| c == code),
            None => false,
        }
    }
}
```

- [ ] **Step 5: Run green + clippy + commit**

Run: `cargo test -p constella-hub-module-country-restrict 2>&1 | tail -6` (Expected PASS)
Run: `cargo clippy -p constella-hub-module-country-restrict --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/hub-module-country-restrict
git commit -m "feat(hub): token-keyed CountryRestrict module (per-token identity + allow-list)"
```

---

### Task 2: Extend `module-interface` with `CountryRestrictClient`

**Files:**
- Modify: `crates/module-interface/src/lib.rs`

**Interfaces:**
- Produces: `CountryRestrictClient` with `configure(token, identity, allowed: Vec<u32>)` / `set_allowed(token, allowed: Vec<u32>)` / `allowed(token) -> Vec<u32>` / `identity(token) -> Address`.

- [ ] **Step 1: Add the trait** — append to `crates/module-interface/src/lib.rs` (beside `MaxBalanceAdmin`)
```rust
/// Config surface of the multi-tenant CountryRestrict module, called by the hub. Token-keyed.
#[contractclient(name = "CountryRestrictClient")]
pub trait CountryRestrictAdmin {
    fn configure(env: Env, token: Address, identity: Address, allowed: Vec<u32>);
    fn set_allowed(env: Env, token: Address, allowed: Vec<u32>);
    fn allowed(env: Env, token: Address) -> Vec<u32>;
    fn identity(env: Env, token: Address) -> Address;
}
```
(Confirm `Vec` is already imported in the file's `use soroban_sdk::{...}`; add it if not.)

- [ ] **Step 2: Build + commit**

Run: `cargo build -p constella-module-interface 2>&1 | tail -1` ; `cargo clippy -p constella-module-interface --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/module-interface/src/lib.rs
git commit -m "feat(interface): CountryRestrictClient for the hub's identity-dependent module"
```

---

### Task 3: Hub — per-token identity deploy + wire CountryRestrict (TDD)

**Files:**
- Modify: `crates/hub/src/lib.rs`, `crates/hub/src/test.rs`

**Interfaces:**
- Consumes: `CountryRestrictClient` (Task 2); the hub's `deploy`/`register`/`module_addr`/`require_token_admin`/`hooks`.
- Produces: platform config `set_identity_wasm(hash: BytesN<32>)`; `LaunchConfig.country_restrict: Vec<u32>` (empty = not selected); `launch` deploys a per-token identity (admin = issuer) when country_restrict is non-empty, stores `Identity(token)`, registers the module on CanCreate+CanTransfer, and calls `configure(token, identity, allowed)`; reads `identity(token) -> Address`; forwarder `set_country_allow(token, codes: Vec<u32>)` (`Admin(token).require_auth`).

- [ ] **Step 1: Write the failing end-to-end isolation test** — append to `crates/hub/src/test.rs`

Add `contractimport!` for the country-restrict + identity wasm at the top:
```rust
mod country_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_country_restrict.wasm"); }
mod identity_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_identity_mock.wasm"); }
```
```rust
#[test]
fn two_tokens_country_restrict_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let identity_hash: BytesN<32> = env.deployer().upload_contract_wasm(identity_wasm::WASM);
    hub.set_identity_wasm(&identity_hash);
    let country = env.register(country_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "country_restrict"), &country);

    // token A allows US(840); token B allows DE(276)
    let ta = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env, 840u32] }).token;
    let tb = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env, 276u32] }).token;

    // Each token got its own identity; attest alice as US on A's, TR on B's.
    let id_a = identity_wasm::Client::new(&env, &hub.identity(&ta));
    let id_b = identity_wasm::Client::new(&env, &hub.identity(&tb));
    let alice = Address::generate(&env);
    id_a.set_country(&alice, &840);
    id_b.set_country(&alice, &792);

    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    tok_a.mint(&alice, &100);            // US ∈ {US} on A -> ok
    assert_eq!(tok_a.balance(&alice), 100);
    assert!(tok_b.try_mint(&alice, &100).is_err()); // TR ∉ {DE} on B -> denied (isolated)
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub two_tokens_country 2>&1 | tail -12`
Expected: FAIL — no `set_identity_wasm`/`identity`/`country_restrict` field.

- [ ] **Step 3: Implement the hub changes** — `crates/hub/src/lib.rs`

Add `country_restrict: Vec<u32>` to `LaunchConfig` (and `use soroban_sdk::Vec` if not present):
```rust
#[contracttype]
#[derive(Clone)]
pub struct LaunchConfig {
    pub admin: Address,
    pub denylist: bool,
    pub max_balance: i128,
    pub country_restrict: Vec<u32>, // empty = not selected
}
```
Add `IdentityWasm` and `Identity(Address)` to the `DataKey` enum. Add the platform setter:
```rust
    pub fn set_identity_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::IdentityWasm, &hash);
    }
```
In `launch`, after the max_balance block, add:
```rust
        if !config.country_restrict.is_empty() {
            let identity_hash: BytesN<32> = env.storage().instance().get(&DataKey::IdentityWasm).unwrap();
            let identity = Self::deploy(&env, &identity_hash, (config.admin.clone(),));
            env.storage().persistent().set(&DataKey::Identity(token.clone()), &identity);
            let m = Self::module_addr(&env, "country_restrict");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            CountryRestrictClient::new(&env, &m).configure(&token, &identity, &config.country_restrict);
        }
```
Add the read + forwarder to the impl block:
```rust
    pub fn identity(env: Env, token: Address) -> Address {
        env.storage().persistent().get(&DataKey::Identity(token)).unwrap()
    }
    pub fn set_country_allow(env: Env, token: Address, codes: Vec<u32>) {
        Self::require_token_admin(&env, &token);
        CountryRestrictClient::new(&env, &Self::module_addr(&env, "country_restrict")).set_allowed(&token, &codes);
    }
```
Extend the module-interface `use` to include `CountryRestrictClient`.

- [ ] **Step 4: Run green**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -8`
Expected: PASS — the country-restrict e2e (alice US-attested passes on token A; same alice TR-attested denied on token B — per-token identity + allow-list isolation) plus all prior hub tests. Update all existing `LaunchConfig` literals in `test.rs` to add `country_restrict: soroban_sdk::vec![&env]` (empty).

- [ ] **Step 5: Negative-auth forwarder test + fmt/clippy/README/commit**
```rust
#[test]
#[should_panic]
fn only_token_admin_can_set_country_allow() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let identity_hash: BytesN<32> = env.deployer().upload_contract_wasm(identity_wasm::WASM);
    hub.set_identity_wasm(&identity_hash);
    let country = env.register(country_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "country_restrict"), &country);
    let t = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env, 840u32] }).token;
    env.set_auths(&[]);
    hub.set_country_allow(&t, &soroban_sdk::vec![&env, 276u32]);
}
```
Update `crates/hub/README.md` (CountryRestrict + the per-token identity model + that the issuer attests directly on `identity(token)`). Then:
```bash
cargo fmt -p constella-hub
cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub
git commit -m "feat(hub): per-token identity deploy + wire CountryRestrict + isolation e2e"
```

---

### Task 4: Workspace + testnet identity/country enforcement (controller spike)

- [ ] **Step 1: Workspace gate**
```bash
stellar contract build >/dev/null
cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"
cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
```

- [ ] **Step 2: Testnet — launch with country_restrict, attest, enforce (controller runs)**

Upload token + country-restrict + identity-mock wasm; deploy hub; deploy the shared country-restrict module (`--hub`); `set_token_wasm`, `set_identity_wasm`, `set_module_addr country_restrict`. A funded issuer launches ONE signed tx with `country_restrict: [840]`. Read `hub.identity(token)` → the per-token identity; the issuer (its admin) attests an account as US(840) and another as TR(792) on that identity. Then mint to the US account (passes) and the TR account (reverts — country not allowed). Record the one-signature launch tx + the attest + the allowed/denied mints. Append evidence to `crates/hub/README.md`.
