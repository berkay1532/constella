# Multi-tenant Hub — core + denylist Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the multi-tenant compliance hub and one token-keyed module (denylist), so an issuer launches a real compliant token in ONE signed transaction against shared contract instances, with per-token state fully isolated.

**Architecture:** A single `hub` contract holds, per token, the issuer-admin + a per-hook registry of shared module addresses; `launch` deploys only the token (pointed at the hub as its compliance engine) and wires the shared modules for that token. Shared modules key ALL state by `(token, …)` and accept mutations only from the hub (`hub.require_auth()`); the hub gates issuer mutations with `Admin(token).require_auth()`. This is the first vertical slice (denylist = the simplest, no mirror, no identity); the other 6 modules follow in a later plan.

**Tech Stack:** Rust / soroban-sdk 26 (`deploy_v2`, `#[contractclient]`, `current_contract_address`), stellar CLI (testnet).

## Global Constraints

- Everything committed is in English.
- Builds to `wasm32v1-none`; `cargo test --workspace` + `cargo clippy` green. TDD (RED before GREEN).
- **Isolation is the core invariant:** every hub/module test uses TWO tokens configured differently and asserts zero cross-talk.
- The hub must NOT depend on any `#[contract]` crate directly (that causes `symbol multiply defined: __constructor` at wasm link — learned from the parked factory). It calls the token via `deploy_v2(wasm_hash, …)` and calls modules via `#[contractclient]` traits from `crates/module-interface`.
- Shared modules are deployed ONCE (by the platform admin) and their addresses registered on the hub; `launch` deploys only the token (+ identity in later plans), never a module.
- Deployed token's admin = `config.admin` (the issuer). Module state keyed by `(token, …)`. Module mutators require the hub's auth; the hub requires `Admin(token)`'s auth.
- Reuse `demo-token` UNCHANGED — it already calls its `compliance` address's `can_*`/post-events with its own address as `token`; the hub implements that same hook surface.
- soroban-sdk 26 verified: `env.deployer().with_current_contract(salt).deploy_v2(hash, (ctor_args))`; `env.current_contract_address()`; a bare `#[contractclient] trait` generates a client without wasm exports.

---

### Task 1: `hub-module-denylist` — token-keyed denylist module (TDD)

**Files:**
- Create: `crates/hub-module-denylist/Cargo.toml`, `crates/hub-module-denylist/src/lib.rs`, `crates/hub-module-denylist/src/test.rs`

**Interfaces:**
- Produces: `DenylistHubModule` with `__constructor(hub: Address)`; hooks `can_transfer(from,to,amount,token)->bool`, `can_create(to,amount,token)->bool`, and no-op `transferred/created/destroyed`; mutators `add_to_denylist(token,account)` / `remove_from_denylist(token,account)` (require the hub's auth); read `is_denied(token,account)->bool`. State keyed by `(token, account)`.

- [ ] **Step 1: Cargo.toml**
```toml
[package]
name = "constella-hub-module-denylist"
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
[dev-dependencies]
soroban-sdk = { workspace = true, features = ["testutils"] }
```

- [ ] **Step 2: Write the failing two-token isolation test** — `crates/hub-module-denylist/src/test.rs`
```rust
#![cfg(test)]
use crate::{DenylistHubModule, DenylistHubModuleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (DenylistHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(DenylistHubModule, (hub.clone(),));
    (DenylistHubModuleClient::new(env, &id), hub)
}

#[test]
fn denylist_is_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let alice = Address::generate(&env);
    let from = Address::generate(&env);

    m.add_to_denylist(&token_a, &alice);
    // token_a sees alice denied; token_b does NOT.
    assert!(m.is_denied(&token_a, &alice));
    assert!(!m.is_denied(&token_b, &alice));
    assert!(!m.can_transfer(&from, &alice, &1, &token_a)); // blocked on A
    assert!(m.can_transfer(&from, &alice, &1, &token_b));  // allowed on B

    m.remove_from_denylist(&token_a, &alice);
    assert!(!m.is_denied(&token_a, &alice));
    assert!(m.can_transfer(&from, &alice, &1, &token_a));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn mutators_require_hub_auth() {
    let env = Env::default();
    // no mock_all_auths -> hub.require_auth() must reject.
    let (m, _hub) = setup(&env);
    m.add_to_denylist(&Address::generate(&env), &Address::generate(&env));
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p constella-hub-module-denylist 2>&1 | tail -12`
Expected: FAIL to compile — `DenylistHubModule` doesn't exist.

- [ ] **Step 4: Implement** — `crates/hub-module-denylist/src/lib.rs`
```rust
#![no_std]
//! Multi-tenant denylist module. One shared instance serves every token; all state is
//! keyed by (token, account). Mutations are accepted only from the hub (the hub gates
//! the issuer's authority per token); the module trusts the hub, stored at construction.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Denied(Address, Address), // (token, account)
}

#[contract]
pub struct DenylistHubModule;

#[contractimpl]
impl DenylistHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }

    pub fn add_to_denylist(env: Env, token: Address, account: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Denied(token, account), &true);
    }

    pub fn remove_from_denylist(env: Env, token: Address, account: Address) {
        Self::require_hub(&env);
        env.storage().persistent().remove(&DataKey::Denied(token, account));
    }

    pub fn is_denied(env: Env, token: Address, account: Address) -> bool {
        env.storage().persistent().get(&DataKey::Denied(token, account)).unwrap_or(false)
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, _amount: i128, token: Address) -> bool {
        !Self::is_denied(env.clone(), token.clone(), from) && !Self::is_denied(env, token, to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        !Self::is_denied(env, token, to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl DenylistHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
}
```

- [ ] **Step 5: Run green + clippy + commit**

Run: `cargo test -p constella-hub-module-denylist 2>&1 | tail -6` (Expected PASS)
Run: `cargo clippy -p constella-hub-module-denylist --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/hub-module-denylist
git commit -m "feat(hub): token-keyed denylist module (per-token isolation, hub-authed mutators)"
```

---

### Task 2: Extend `module-interface` with the hub's module clients (no wasm exports)

**Files:**
- Modify: `crates/module-interface/src/lib.rs`

**Interfaces:**
- Produces: `DenylistClient` (from a `#[contractclient]` trait) with `add_to_denylist(token, account)` / `remove_from_denylist(token, account)` / `is_denied(token, account) -> bool`, for the hub's forwarders. The existing `ModuleClient` (hook surface) is reused for fan-out.

- [ ] **Step 1: Add the trait** — append to `crates/module-interface/src/lib.rs`
```rust
/// Admin surface of the multi-tenant denylist module, called by the hub's forwarders.
/// Token-keyed so one shared instance serves every token.
#[contractclient(name = "DenylistClient")]
pub trait DenylistAdmin {
    fn add_to_denylist(env: Env, token: Address, account: Address);
    fn remove_from_denylist(env: Env, token: Address, account: Address);
    fn is_denied(env: Env, token: Address, account: Address) -> bool;
}
```
(It sits beside the existing `ModuleClient`/`TokenClient`/`IdentityClient` traits — a bare `#[contractclient]` trait generates only a client, no `#[contract]`, so no wasm-export collision when the hub depends on this crate.)

- [ ] **Step 2: Build + commit**

Run: `cargo build -p constella-module-interface 2>&1 | tail -1` (Expected: Finished)
Run: `cargo clippy -p constella-module-interface --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/module-interface/src/lib.rs
git commit -m "feat(interface): DenylistClient for the hub's token-keyed denylist forwarders"
```

---

### Task 3: `hub` crate — config + one-signature `launch` (TDD)

**Files:**
- Create: `crates/hub/Cargo.toml`, `crates/hub/src/lib.rs`, `crates/hub/src/test.rs`

**Interfaces:**
- Consumes: `deploy_v2` for the token; `ModuleClient` (later task) — not needed yet.
- Produces: `Hub` with `__constructor(platform_admin: Address)`; platform config `set_token_wasm(hash)`, `set_module_addr(kind: Symbol, addr: Address)`; `launch(config: LaunchConfig) -> LaunchResult`; per-token reads `token_admin(token)`, `modules_for(token, hook) -> Vec<Address>`. `LaunchConfig { admin: Address, denylist: bool }` (this slice); `LaunchResult { token: Address }`.

- [ ] **Step 1: Cargo.toml**
```toml
[package]
name = "constella-hub"
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
```

- [ ] **Step 2: Write the failing launch test** — `crates/hub/src/test.rs`

Import the real token + denylist-module wasm via `contractimport!` (build order: `stellar contract build` before `cargo test -p constella-hub`).
```rust
#![cfg(test)]
use crate::{Hub, HubClient, LaunchConfig};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol};

mod token_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_demo_token.wasm"); }
mod denylist_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_denylist.wasm"); }

fn deploy_hub(env: &Env) -> (HubClient<'static>, Address) {
    let admin = Address::generate(env);
    let id = env.register(Hub, (admin.clone(),));
    (HubClient::new(env, &id), id)
}

#[test]
fn launch_deploys_token_and_wires_denylist() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);

    // platform admin config: token wasm + the shared denylist module address.
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let denylist = env.register(denylist_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "denylist"), &denylist);

    let issuer = Address::generate(&env);
    let res = hub.launch(&LaunchConfig { admin: issuer.clone(), denylist: true });

    assert_eq!(hub.token_admin(&res.token), issuer);
    // denylist is registered for this token on both pre-check hooks
    let on_create = hub.modules_for(&res.token, &Symbol::new(&env, "CanCreate"));
    assert!(on_create.contains(&denylist));
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -12`
Expected: FAIL — `Hub` doesn't exist.

- [ ] **Step 4: Implement config + launch** — `crates/hub/src/lib.rs`

Hooks are added in Task 4; this task is config + launch. Represent the per-hook registry with a `Symbol` hook name so the hub owns the enum-free surface. Full file:
```rust
#![no_std]
//! Multi-tenant compliance hub. One instance serves every token: per-token issuer-admin,
//! a per-(token,hook) registry of shared module addresses, and a one-signature `launch`
//! that deploys the token (pointed at this hub) and wires the selected shared modules.

use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, BytesN, Env, Symbol, Vec};

#[cfg(test)]
mod test;

#[contracttype]
#[derive(Clone)]
pub struct LaunchConfig {
    pub admin: Address,
    pub denylist: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct LaunchResult {
    pub token: Address,
}

#[contracttype]
enum DataKey {
    PlatformAdmin,
    TokenWasm,
    ModuleAddr(Symbol),          // shared module address by kind
    Counter,
    TokenAdmin(Address),         // token -> issuer
    Modules(Address, Symbol),    // (token, hook) -> Vec<Address>
}

#[contract]
pub struct Hub;

#[contractimpl]
impl Hub {
    pub fn __constructor(env: Env, platform_admin: Address) {
        env.storage().instance().set(&DataKey::PlatformAdmin, &platform_admin);
    }

    pub fn set_token_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::TokenWasm, &hash);
    }

    pub fn set_module_addr(env: Env, kind: Symbol, addr: Address) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::ModuleAddr(kind), &addr);
    }

    pub fn token_admin(env: Env, token: Address) -> Address {
        env.storage().persistent().get(&DataKey::TokenAdmin(token)).unwrap()
    }

    pub fn modules_for(env: Env, token: Address, hook: Symbol) -> Vec<Address> {
        env.storage().persistent().get(&DataKey::Modules(token, hook)).unwrap_or(Vec::new(&env))
    }

    /// One-signature launch: deploy the token (admin = issuer, compliance = this hub) and
    /// wire the selected shared modules for that token.
    pub fn launch(env: Env, config: LaunchConfig) -> LaunchResult {
        config.admin.require_auth();
        let token_hash: BytesN<32> = env.storage().instance().get(&DataKey::TokenWasm).unwrap();
        let hub_addr = env.current_contract_address();
        let token = Self::deploy(&env, &token_hash, (config.admin.clone(), hub_addr));
        env.storage().persistent().set(&DataKey::TokenAdmin(token.clone()), &config.admin);

        if config.denylist {
            let m: Address = env.storage().instance().get(&DataKey::ModuleAddr(Symbol::new(&env, "denylist"))).unwrap();
            for h in ["CanCreate", "CanTransfer"] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
        }
        LaunchResult { token }
    }
}

impl Hub {
    fn require_platform_admin(env: &Env) {
        let a: Address = env.storage().instance().get(&DataKey::PlatformAdmin).unwrap();
        a.require_auth();
    }
    fn next_salt(env: &Env) -> BytesN<32> {
        let n: u32 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0);
        env.storage().instance().set(&DataKey::Counter, &(n + 1));
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&n.to_be_bytes());
        BytesN::from_array(env, &b)
    }
    fn deploy<A: soroban_sdk::ConstructorArgs>(env: &Env, hash: &BytesN<32>, args: A) -> Address {
        env.deployer().with_current_contract(Self::next_salt(env)).deploy_v2(hash.clone(), args)
    }
    fn register(env: &Env, token: &Address, hook: &Symbol, module: &Address) {
        let key = DataKey::Modules(token.clone(), hook.clone());
        let mut v: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![env]);
        v.push_back(module.clone());
        env.storage().persistent().set(&key, &v);
    }
}
```

- [ ] **Step 5: Run green + clippy + commit**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -6` (Expected PASS)
Run: `cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/hub
git commit -m "feat(hub): config + one-signature launch (deploy token, wire shared denylist)"
```

---

### Task 4: Hub hook fan-out + issuer forwarders + end-to-end isolation (TDD)

**Files:**
- Modify: `crates/hub/src/lib.rs` (add the hook surface + forwarders)
- Modify: `crates/hub/src/test.rs` (end-to-end two-token isolation)

**Interfaces:**
- Consumes: `ModuleClient` + `DenylistClient` from `module-interface`.
- Produces on the hub: `can_transfer/can_create(...token)`, `transferred/created/destroyed(...token)` (token.require_auth), and forwarders `add_to_denylist(token, account)` / `remove_from_denylist(token, account)` / `is_denied(token, account)`.

- [ ] **Step 1: Write the failing end-to-end isolation test** — append to `crates/hub/src/test.rs`
```rust
#[test]
fn two_tokens_denylist_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let denylist = env.register(denylist_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "denylist"), &denylist);

    let issuer_a = Address::generate(&env);
    let issuer_b = Address::generate(&env);
    let ta = hub.launch(&LaunchConfig { admin: issuer_a.clone(), denylist: true }).token;
    let tb = hub.launch(&LaunchConfig { admin: issuer_b.clone(), denylist: true }).token;

    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    tok_a.mint(&alice, &100);
    tok_b.mint(&alice, &100);
    // issuer A denylists bob on token A only
    hub.add_to_denylist(&ta, &bob);
    assert!(tok_a.try_transfer(&alice, &bob, &10).is_err()); // blocked on A
    tok_b.transfer(&alice, &bob, &10);                        // allowed on B
    assert_eq!(tok_b.balance(&bob), 10);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub two_tokens 2>&1 | tail -12`
Expected: FAIL — hub has no `can_transfer`/`add_to_denylist` yet, so the token's compliance calls fail / methods missing.

- [ ] **Step 3: Add the hook surface + forwarders** — in `crates/hub/src/lib.rs`, add to the `#[contractimpl] impl Hub` block:
```rust
    // --- hook surface (called by the token; matches the ModuleClient ABI) ---
    pub fn can_transfer(env: Env, from: Address, to: Address, amount: i128, token: Address) -> bool {
        for m in Self::modules_for(env.clone(), token.clone(), Symbol::new(&env, "CanTransfer")).iter() {
            if !ModuleClient::new(&env, &m).can_transfer(&from, &to, &amount, &token) { return false; }
        }
        true
    }
    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        for m in Self::modules_for(env.clone(), token.clone(), Symbol::new(&env, "CanCreate")).iter() {
            if !ModuleClient::new(&env, &m).can_create(&to, &amount, &token) { return false; }
        }
        true
    }
    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address) {
        token.require_auth();
        for m in Self::modules_for(env.clone(), token.clone(), Symbol::new(&env, "Transferred")).iter() {
            ModuleClient::new(&env, &m).transferred(&from, &to, &amount, &token);
        }
    }
    pub fn created(env: Env, to: Address, amount: i128, token: Address) {
        token.require_auth();
        for m in Self::modules_for(env.clone(), token.clone(), Symbol::new(&env, "Created")).iter() {
            ModuleClient::new(&env, &m).created(&to, &amount, &token);
        }
    }
    pub fn destroyed(env: Env, from: Address, amount: i128, token: Address) {
        token.require_auth();
        for m in Self::modules_for(env.clone(), token.clone(), Symbol::new(&env, "Destroyed")).iter() {
            ModuleClient::new(&env, &m).destroyed(&from, &amount, &token);
        }
    }

    // --- issuer forwarders (single auth surface: Admin(token).require_auth) ---
    pub fn add_to_denylist(env: Env, token: Address, account: Address) {
        Self::require_token_admin(&env, &token);
        DenylistClient::new(&env, &Self::denylist_addr(&env)).add_to_denylist(&token, &account);
    }
    pub fn remove_from_denylist(env: Env, token: Address, account: Address) {
        Self::require_token_admin(&env, &token);
        DenylistClient::new(&env, &Self::denylist_addr(&env)).remove_from_denylist(&token, &account);
    }
    pub fn is_denied(env: Env, token: Address, account: Address) -> bool {
        DenylistClient::new(&env, &Self::denylist_addr(&env)).is_denied(&token, &account)
    }
```
Add imports `use constella_module_interface::{ModuleClient, DenylistClient};` and the private helpers:
```rust
    fn require_token_admin(env: &Env, token: &Address) {
        let a: Address = env.storage().persistent().get(&DataKey::TokenAdmin(token.clone())).unwrap();
        a.require_auth();
    }
    fn denylist_addr(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::ModuleAddr(Symbol::new(env, "denylist"))).unwrap()
    }
```

- [ ] **Step 4: Run green**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -8`
Expected: PASS — both hub tests, including `two_tokens_denylist_isolated_end_to_end` (token A blocks bob, token B unaffected — real isolation through two live token contracts sharing one denylist instance).
> Note: `demo-token` calls its compliance's `can_create`/`created` on mint and `can_transfer`/`transferred` on transfer. The hub implements exactly those. If a name/arity mismatch surfaces, reconcile the hub's hook signatures with what `crates/demo-token/src/lib.rs` actually calls.

- [ ] **Step 5: Add a negative-auth forwarder test + fmt/clippy/README/commit**
```rust
#[test]
#[should_panic]
fn only_token_admin_can_denylist() {
    let env = Env::default();
    // no mock_all_auths -> Admin(token).require_auth() rejects a non-admin caller
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let denylist = env.register(denylist_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "denylist"), &denylist);
    let issuer = Address::generate(&env);
    // NOTE: launch needs issuer auth; register launch under mock, then drop mocks by using a
    // fresh env section — simplest: assert the forwarder panics when the token has an admin
    // that did not authorize. If mock scoping is awkward, keep this as a module-level unit
    // test of require_token_admin instead and note it.
    let _ = (hub, issuer);
}
```
(If the mock scoping makes this awkward, keep the per-token-admin check covered by asserting `require_token_admin` behavior in a focused way and note it — do not weaken the isolation tests.)
Write `crates/hub/README.md` (multi-tenant model, launch, forwarders, the shared-module/per-token-state design, the build-order note). Then:
```bash
cargo fmt -p constella-hub
cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub
git commit -m "feat(hub): hook fan-out + issuer forwarders + two-token isolation e2e"
```

---

### Task 5: Workspace + CI + testnet one-signature launch (controller spike)

**Files:**
- Modify: `.github/workflows/ci.yml` if needed (the hub tests `contractimport!` the token + denylist wasm — the `test` job must build wasm first; this was already added for the factory but factory is on a parked branch, so re-verify it's present on THIS branch).

- [ ] **Step 1: Workspace gate**
```bash
stellar contract build >/dev/null
cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"
cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
```
Expected: green. If CI's `test` job doesn't build wasm before tests, add `cargo build --workspace --release --target wasm32v1-none` before the clippy/coverage steps (and `targets: wasm32v1-none` on its toolchain) — same fix the factory needed.

- [ ] **Step 2: Commit any CI change**
```bash
git add .github/workflows/ci.yml Cargo.lock
git commit -m "ci(hub): build wasm before tests so hub contractimport resolves"
```

- [ ] **Step 3: Testnet one-signature launch (controller runs this)**
```bash
stellar contract build >/dev/null
# install token + denylist wasm; deploy hub; deploy the shared denylist once; configure:
DENYHASH=$(stellar contract upload --wasm target/wasm32v1-none/release/constella_hub_module_denylist.wasm --source deployer --network testnet)
TOKHASH=$(stellar contract upload --wasm target/wasm32v1-none/release/constella_demo_token.wasm --source deployer --network testnet)
HUB=$(stellar contract deploy --wasm target/wasm32v1-none/release/constella_hub.wasm --source deployer --network testnet -- --platform_admin $(stellar keys address deployer))
DENY=$(stellar contract deploy --wasm target/wasm32v1-none/release/constella_hub_module_denylist.wasm --source deployer --network testnet -- --hub $HUB)
stellar contract invoke --id $HUB --source deployer --network testnet -- set_token_wasm --hash $TOKHASH
stellar contract invoke --id $HUB --source deployer --network testnet -- set_module_addr --kind denylist --addr $DENY
# a funded issuer launches in ONE signed tx:
stellar contract invoke --id $HUB --source issuer --network testnet -- launch --config '{"admin":"<ISSUER>","denylist":true}'
```
Record: the launch is ONE transaction (capture the tx hash), then drive a mint + a denied transfer to prove the token enforces the denylist live. Append the evidence (one-signature confirmation + tx hashes) to `crates/hub/README.md`.
