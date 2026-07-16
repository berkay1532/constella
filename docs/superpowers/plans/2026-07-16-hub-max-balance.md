# Multi-tenant Hub — add MaxBalance (stateful mirror) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a token-keyed **stateful** module (MaxBalance) to the multi-tenant hub, validating the post-event / balance-mirror path (which the denylist slice did not exercise) with full per-token isolation.

**Architecture:** A shared `hub-module-max-balance` keys its balance mirror + per-token cap by `(token, …)`; it updates the mirror only from the hub's post-event fan-out (`hub.require_auth()`), and enforces `bal(token,to)+amount <= max(token)` on pre-checks. The hub registers it on all five hooks per token, initializes the per-token cap at `launch`, and exposes an issuer forwarder `set_max_balance(token, cap)`.

**Tech Stack:** Rust / soroban-sdk 26, stellar CLI (testnet).

## Global Constraints

- English; builds to `wasm32v1-none`; `cargo test --workspace` + `cargo clippy` green. TDD (RED before GREEN).
- **Per-token isolation invariant:** every stateful key carries `token`; two tokens with different caps/balances never affect each other. Every test uses two tokens.
- The hub must NOT depend on any `#[contract]` crate; calls modules via `#[contractclient]` traits from `module-interface` (`ModuleClient` for hooks, a new `MaxBalanceClient` for config).
- Module mutators + post-events require the hub's auth (`hub.require_auth()`); the hub's post-events require `token.require_auth()` (already built). Guard tests pinned to `#[should_panic(expected = "Error(Auth, InvalidAction)")]`.
- Reuse the hub's existing `mod hooks` constants (`CAN_CREATE`/`CAN_TRANSFER`/`TRANSFERRED`/`CREATED`/`DESTROYED`) on both the register and fan-out sides — no raw literals.
- Hub tests `contractimport!` the token + module wasm → run `stellar contract build` before `cargo test -p constella-hub`.

---

### Task 1: `hub-module-max-balance` — token-keyed stateful mirror (TDD)

**Files:**
- Create: `crates/hub-module-max-balance/Cargo.toml`, `src/lib.rs`, `src/test.rs`

**Interfaces:**
- Produces: `MaxBalanceHubModule` with `__constructor(hub: Address)`; `set_max(token, cap: i128)` (hub-authed); `max(token) -> i128`; hooks `can_transfer(from,to,amount,token)->bool` / `can_create(to,amount,token)->bool` (enforce `bal(token,to)+amount <= max(token)`); post-events `transferred/created/destroyed(...,token)` (hub-authed, update `Bal(token,holder)`).

- [ ] **Step 1: Cargo.toml** (mirror `crates/hub-module-denylist/Cargo.toml`)
```toml
[package]
name = "constella-hub-module-max-balance"
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

- [ ] **Step 2: Write the failing two-token isolation + guard tests** — `crates/hub-module-max-balance/src/test.rs`
```rust
#![cfg(test)]
use crate::{MaxBalanceHubModule, MaxBalanceHubModuleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (MaxBalanceHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(MaxBalanceHubModule, (hub.clone(),));
    (MaxBalanceHubModuleClient::new(env, &id), hub)
}

#[test]
fn mirror_and_cap_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    let alice = Address::generate(&env);
    m.set_max(&ta, &1000);
    m.set_max(&tb, &500);

    // token A: alice acquires 800 (via post-event fan-out simulated as a direct call under mock)
    m.created(&alice, &800, &ta);
    assert!(!m.can_create(&alice, &300, &ta)); // 800 + 300 > 1000 -> denied on A
    // token B is untouched: alice's B-balance is 0, B cap 500
    assert!(m.can_create(&alice, &300, &tb));  // 0 + 300 <= 500 -> allowed on B
    assert!(m.max(&ta) == 1000 && m.max(&tb) == 500);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn post_event_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env); // no mock_all_auths
    m.created(&Address::generate(&env), &1, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn set_max_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.set_max(&Address::generate(&env), &1);
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p constella-hub-module-max-balance 2>&1 | tail -12`
Expected: FAIL to compile — the type doesn't exist.

- [ ] **Step 4: Implement** — `crates/hub-module-max-balance/src/lib.rs`
```rust
#![no_std]
//! Multi-tenant MaxBalance module: caps the balance any single holder may reach, per
//! token. One shared instance serves every token; the balance mirror and the per-token
//! cap are keyed by (token, …). The mirror is updated only from the hub's post-event
//! fan-out (hub-authed); pre-checks enforce bal(token,to) + amount <= max(token).

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Max(Address),          // token -> cap
    Bal(Address, Address), // (token, holder) -> balance
}

#[contract]
pub struct MaxBalanceHubModule;

#[contractimpl]
impl MaxBalanceHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }

    pub fn set_max(env: Env, token: Address, cap: i128) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Max(token), &cap);
    }

    pub fn max(env: Env, token: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Max(token)).unwrap_or(0)
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, amount: i128, token: Address) -> bool {
        Self::within_cap(&env, &token, &to, amount)
    }

    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        Self::within_cap(&env, &token, &to, amount)
    }

    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::apply(&env, &token, &from, -amount);
        Self::apply(&env, &token, &to, amount);
    }

    pub fn created(env: Env, to: Address, amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::apply(&env, &token, &to, amount);
    }

    pub fn destroyed(env: Env, from: Address, amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::apply(&env, &token, &from, -amount);
    }
}

impl MaxBalanceHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn bal(env: &Env, token: &Address, who: &Address) -> i128 {
        env.storage().persistent().get(&DataKey::Bal(token.clone(), who.clone())).unwrap_or(0)
    }
    fn within_cap(env: &Env, token: &Address, to: &Address, amount: i128) -> bool {
        Self::bal(env, token, to) + amount <= Self::max(env.clone(), token.clone())
    }
    fn apply(env: &Env, token: &Address, who: &Address, delta: i128) {
        if delta == 0 { return; }
        let key = DataKey::Bal(token.clone(), who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(old + delta));
    }
}
```

- [ ] **Step 5: Run green + clippy + commit**

Run: `cargo test -p constella-hub-module-max-balance 2>&1 | tail -6` (Expected PASS)
Run: `cargo clippy -p constella-hub-module-max-balance --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/hub-module-max-balance
git commit -m "feat(hub): token-keyed MaxBalance module (per-token mirror + cap, hub-authed)"
```

---

### Task 2: Extend `module-interface` with `MaxBalanceClient` (config surface)

**Files:**
- Modify: `crates/module-interface/src/lib.rs`

**Interfaces:**
- Produces: `MaxBalanceClient` (from a `#[contractclient]` trait) with `set_max(token, cap: i128)` / `max(token) -> i128`, for the hub's launch init + forwarder. Hooks are reused via the existing `ModuleClient`.

- [ ] **Step 1: Add the trait** — append to `crates/module-interface/src/lib.rs` (beside `DenylistAdmin`)
```rust
/// Config surface of the multi-tenant MaxBalance module, called by the hub (launch init
/// + the issuer forwarder). Token-keyed.
#[contractclient(name = "MaxBalanceClient")]
pub trait MaxBalanceAdmin {
    fn set_max(env: Env, token: Address, cap: i128);
    fn max(env: Env, token: Address) -> i128;
}
```

- [ ] **Step 2: Build + commit**

Run: `cargo build -p constella-module-interface 2>&1 | tail -1` ; `cargo clippy -p constella-module-interface --all-targets 2>&1 | grep -E "warning|error" | head`
```bash
git add crates/module-interface/src/lib.rs
git commit -m "feat(interface): MaxBalanceClient for the hub's max-balance config forwarders"
```

---

### Task 3: Hub — wire MaxBalance into launch + forwarder + config (TDD)

**Files:**
- Modify: `crates/hub/src/lib.rs` (LaunchConfig, launch, new forwarder + read, imports)
- Modify: `crates/hub/src/test.rs` (two-token end-to-end cap isolation)

**Interfaces:**
- Consumes: `MaxBalanceClient` (Task 2), the hub's `mod hooks`, `register`, `require_token_admin`, `ModuleAddr` from prior tasks.
- Produces: `LaunchConfig` gains `max_balance: i128` (0 = not selected). `launch` registers the shared max-balance module on all 5 hooks + calls `set_max(token, cap)`. New: `set_max_balance(token, cap)` forwarder (`Admin(token).require_auth`) + `max_balance(token) -> i128` read.

- [ ] **Step 1: Write the failing end-to-end cap-isolation test** — append to `crates/hub/src/test.rs`

Add a `contractimport!` for the max-balance wasm at the top (beside the existing token/denylist ones):
```rust
mod maxbal_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_max_balance.wasm"); }
```
```rust
#[test]
fn two_tokens_max_balance_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let maxbal = env.register(maxbal_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "max_balance"), &maxbal);

    // token A cap 1000, token B cap 100
    let ta = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 1000 }).token;
    let tb = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 100 }).token;
    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    let alice = Address::generate(&env);

    tok_a.mint(&alice, &900);            // under A's 1000 cap -> ok, mirror updates via hub fan-out
    assert_eq!(tok_a.balance(&alice), 900);
    assert!(tok_a.try_mint(&alice, &200).is_err()); // 900 + 200 > 1000 -> denied on A
    // token B has cap 100 and alice's B-balance is independent (0)
    assert!(tok_b.try_mint(&alice, &200).is_err()); // > B cap 100
    tok_b.mint(&alice, &50);             // under B's 100 cap -> ok
    assert_eq!(tok_b.balance(&alice), 50);
    assert_eq!(hub.max_balance(&ta), 1000);
    assert_eq!(hub.max_balance(&tb), 100);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub two_tokens_max_balance 2>&1 | tail -12`
Expected: FAIL — `LaunchConfig` has no `max_balance` field / hub has no `max_balance`/`set_max_balance`.

- [ ] **Step 3: Implement the hub changes** — `crates/hub/src/lib.rs`

Add `max_balance: i128` to `LaunchConfig`:
```rust
#[contracttype]
#[derive(Clone)]
pub struct LaunchConfig {
    pub admin: Address,
    pub denylist: bool,
    pub max_balance: i128, // 0 = not selected
}
```
In `launch`, after the denylist block, add:
```rust
        if config.max_balance > 0 {
            let m: Address = env.storage().instance()
                .get(&DataKey::ModuleAddr(Symbol::new(&env, "max_balance"))).unwrap();
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER, hooks::CREATED, hooks::TRANSFERRED, hooks::DESTROYED] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            MaxBalanceClient::new(&env, &m).set_max(&token, &config.max_balance);
        }
```
Add imports: extend the module-interface `use` to include `MaxBalanceClient`.
Add the forwarder + read to the `#[contractimpl] impl Hub` block:
```rust
    pub fn set_max_balance(env: Env, token: Address, cap: i128) {
        Self::require_token_admin(&env, &token);
        MaxBalanceClient::new(&env, &Self::module_addr(&env, "max_balance")).set_max(&token, &cap);
    }
    pub fn max_balance(env: Env, token: Address) -> i128 {
        MaxBalanceClient::new(&env, &Self::module_addr(&env, "max_balance")).max(&token)
    }
```
Refactor the existing `denylist_addr` helper into a generic `module_addr(env, kind: &str) -> Address` (and update `denylist_addr`'s call sites to `module_addr(env, "denylist")`), so both modules share one lookup:
```rust
    fn module_addr(env: &Env, kind: &str) -> Address {
        env.storage().instance().get(&DataKey::ModuleAddr(Symbol::new(env, kind))).unwrap()
    }
```

- [ ] **Step 4: Run green**

Run: `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -8`
Expected: PASS — the max-balance e2e (mirror updates via the hub's post-event fan-out; cap enforced per token; token B independent) plus all prior hub tests still green (denylist e2e unaffected — note the existing tests construct `LaunchConfig` without `max_balance`; update those to `max_balance: 0`).

- [ ] **Step 5: Add a negative-auth forwarder test + fmt/clippy/README/commit**
```rust
#[test]
#[should_panic]
fn only_token_admin_can_set_max_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let maxbal = env.register(maxbal_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "max_balance"), &maxbal);
    let t = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 100 }).token;
    env.set_auths(&[]); // drop mocked auths -> the token's issuer did not authorize
    hub.set_max_balance(&t, &999);
}
```
Update `crates/hub/README.md` (add MaxBalance to the module list + the config field). Then:
```bash
cargo fmt -p constella-hub
cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub
git commit -m "feat(hub): wire MaxBalance — per-token cap in launch + issuer forwarder + isolation e2e"
```

---

### Task 4: Workspace + testnet cap-enforcement (controller spike)

- [ ] **Step 1: Workspace gate**
```bash
stellar contract build >/dev/null
cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"
cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
```
Expected: green (CI already builds wasm before tests from the denylist slice — no CI change needed; verify).

- [ ] **Step 2: Testnet — launch with denylist + max_balance in ONE sig, prove the cap enforces live (controller runs)**
```bash
stellar contract build >/dev/null
# upload token + both module wasms; deploy hub; deploy the two shared modules; configure:
MBHASH=... ; # upload constella_hub_module_max_balance.wasm
HUB=... ; DENY=... ; MAXBAL=$(stellar contract deploy --wasm .../constella_hub_module_max_balance.wasm --source deployer --network testnet -- --hub $HUB)
stellar contract invoke --id $HUB --source deployer --network testnet -- set_module_addr --kind max_balance --addr $MAXBAL
# issuer launches a token capped at 1000, ONE signed tx:
stellar contract invoke --id $HUB --source issuer --network testnet -- launch --config '{"admin":"<ISSUER>","denylist":false,"max_balance":"1000"}'
# mint 900 (ok), mint 200 more (should REVERT: 900+200 > 1000)
```
Record: one-signature launch tx hash; a mint under the cap passing and one over the cap reverting (the mirror updated live via the hub's post-event fan-out). Append the evidence to `crates/hub/README.md`.
