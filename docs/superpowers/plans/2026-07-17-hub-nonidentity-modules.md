# Multi-tenant Hub — add MaxHolders, Lockup, TransferWindow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the three remaining non-identity token-keyed modules to the multi-tenant hub — MaxHolders (stateful mirror + count), Lockup (time-based state), TransferWindow (config-only) — all pure applications of the already-proven patterns.

**Architecture:** Each is a shared, token-keyed module (state keyed by `(token, …)`; mutators/post-events require `hub.require_auth()`). The hub registers each on its hooks per token, initializes per-token config at `launch`, and exposes issuer forwarders. No new mechanic — max-holders mirrors the max-balance stateful pattern; transfer-window mirrors the denylist config pattern; lockup is a stateful pattern with a time gate.

**Tech Stack:** Rust / soroban-sdk 26, stellar CLI (testnet).

## Global Constraints

- English; builds to `wasm32v1-none`; `cargo test --workspace` + `cargo clippy` green. TDD (RED before GREEN).
- **Per-token isolation:** every stateful/config key carries `token`; two tokens never affect each other. Every test uses two tokens.
- Hub must NOT depend on any `#[contract]` crate; calls modules via `#[contractclient]` traits from `module-interface`. Reuse the hub's `deploy`/`register`/`module_addr`/`require_token_admin`/`hooks` helpers.
- Module mutators + post-events require `hub.require_auth()`; forwarders require the per-token issuer (`require_token_admin`). Guard tests pinned to `#[should_panic(expected = "Error(Auth, InvalidAction)")]`.
- Reuse the hub's `mod hooks` constants on both register and (for stateful modules) fan-out sides.
- Hub tests `contractimport!` all module wasm → run `stellar contract build` before `cargo test -p constella-hub`.
- Selection sentinels in `LaunchConfig`: `max_holders: u32` (0 = off), `lockup: u64` (0 = off — a 0-second lock is a no-op anyway), `transfer_window: bool`.

---

### Task 1: `hub-module-max-holders` — token-keyed stateful mirror + count (TDD)

**Files:** Create `crates/hub-module-max-holders/{Cargo.toml, src/lib.rs, src/test.rs}`

**Interfaces:** `MaxHoldersHubModule` — `__constructor(hub)`; `set_max(token, cap: u32)` (hub-authed); reads `max(token)->u32`, `holders(token)->u32`; hooks `can_transfer`/`can_create` (allow if `bal(token,to)>0` OR `count(token)<max(token)`); post-events (hub-authed) maintain `Bal(token,holder)` + `Count(token)` with 0-crossing.

- [ ] **Step 1: Cargo.toml** (mirror `crates/hub-module-max-balance/Cargo.toml`, name `constella-hub-module-max-holders`). Deps: soroban-sdk (+ dev testutils).

- [ ] **Step 2: Failing tests** — `src/test.rs`
```rust
#![cfg(test)]
use crate::{MaxHoldersHubModule, MaxHoldersHubModuleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (MaxHoldersHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(MaxHoldersHubModule, (hub.clone(),));
    (MaxHoldersHubModuleClient::new(env, &id), hub)
}

#[test]
fn count_and_cap_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.set_max(&ta, &2);
    m.set_max(&tb, &1);
    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    m.created(&a1, &100, &ta);
    m.created(&a2, &100, &ta);
    assert_eq!(m.holders(&ta), 2);
    let a3 = Address::generate(&env);
    assert!(!m.can_create(&a3, &1, &ta)); // token A full at 2
    // token B independent: its count is 0
    assert!(m.can_create(&a3, &1, &tb));  // room under B's cap 1
    // existing holder always allowed
    assert!(m.can_create(&a1, &1, &ta));
    // free a slot on A
    m.destroyed(&a1, &100, &ta);
    assert_eq!(m.holders(&ta), 1);
    assert!(m.can_create(&a3, &1, &ta));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn post_event_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
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

- [ ] **Step 3: Run RED** — `cargo test -p constella-hub-module-max-holders 2>&1 | tail -12` (compile fail).

- [ ] **Step 4: Implement** — `src/lib.rs`
```rust
#![no_std]
//! Multi-tenant MaxHolders module: caps the number of distinct holders per token.
//! One shared instance; balance mirror + holder count keyed by (token, …), updated only
//! from the hub's post-event fan-out (hub-authed).

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Max(Address),          // token -> cap
    Count(Address),        // token -> distinct holders
    Bal(Address, Address), // (token, holder) -> balance
}

#[contract]
pub struct MaxHoldersHubModule;

#[contractimpl]
impl MaxHoldersHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn set_max(env: Env, token: Address, cap: u32) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Max(token), &cap);
    }
    pub fn max(env: Env, token: Address) -> u32 {
        env.storage().persistent().get(&DataKey::Max(token)).unwrap_or(0)
    }
    pub fn holders(env: Env, token: Address) -> u32 {
        env.storage().persistent().get(&DataKey::Count(token)).unwrap_or(0)
    }
    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, token: Address) -> bool {
        Self::allows(&env, &token, &to)
    }
    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        Self::allows(&env, &token, &to)
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

impl MaxHoldersHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn bal(env: &Env, token: &Address, who: &Address) -> i128 {
        env.storage().persistent().get(&DataKey::Bal(token.clone(), who.clone())).unwrap_or(0)
    }
    fn allows(env: &Env, token: &Address, to: &Address) -> bool {
        if Self::bal(env, token, to) > 0 { return true; }
        let count: u32 = env.storage().persistent().get(&DataKey::Count(token.clone())).unwrap_or(0);
        let max: u32 = env.storage().persistent().get(&DataKey::Max(token.clone())).unwrap_or(0);
        count < max
    }
    fn apply(env: &Env, token: &Address, who: &Address, delta: i128) {
        if delta == 0 { return; }
        let key = DataKey::Bal(token.clone(), who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new = old + delta;
        env.storage().persistent().set(&key, &new);
        let ckey = DataKey::Count(token.clone());
        let mut count: u32 = env.storage().persistent().get(&ckey).unwrap_or(0);
        if old == 0 && new > 0 { count += 1; env.storage().persistent().set(&ckey, &count); }
        else if old > 0 && new == 0 { count -= 1; env.storage().persistent().set(&ckey, &count); }
    }
}
```

- [ ] **Step 5: Run GREEN + clippy + commit**
```bash
cargo test -p constella-hub-module-max-holders 2>&1 | tail -6
cargo clippy -p constella-hub-module-max-holders --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub-module-max-holders
git commit -m "feat(hub): token-keyed MaxHolders module (per-token mirror + holder count)"
```

---

### Task 2: `hub-module-lockup` — token-keyed time-based state (TDD)

**Files:** Create `crates/hub-module-lockup/{Cargo.toml, src/lib.rs, src/test.rs}`

**Interfaces:** `LockupHubModule` — `__constructor(hub)`; `set_duration(token, secs: u64)` (hub-authed); read `unlock_at(token, holder)->u64`; `can_transfer` gates on `now >= acquired + duration(token)` (allowed if never acquired); `can_create` always true; post-events `transferred`/`created` (hub-authed) record `Acquired(token,holder) = now`; `destroyed` no-op.

- [ ] **Step 1: Cargo.toml** (name `constella-hub-module-lockup`; deps soroban-sdk + dev testutils).

- [ ] **Step 2: Failing tests** — `src/test.rs`
```rust
#![cfg(test)]
use crate::{LockupHubModule, LockupHubModuleClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

fn setup(env: &Env) -> (LockupHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(LockupHubModule, (hub.clone(),));
    (LockupHubModuleClient::new(env, &id), hub)
}

#[test]
fn lock_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);
    let (m, _hub) = setup(&env);
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.set_duration(&ta, &100); // A: 100s lock
    m.set_duration(&tb, &0);   // B: no lock
    let alice = Address::generate(&env);
    m.created(&alice, &10, &ta); // acquired at 1000 on A
    let tok = Address::generate(&env);
    assert!(!m.can_transfer(&alice, &Address::generate(&env), &1, &ta)); // 1000 < 1000+100 -> locked on A
    // token B: alice never acquired there -> not locked
    assert!(m.can_transfer(&alice, &Address::generate(&env), &1, &tb));
    let _ = tok;
    env.ledger().set_timestamp(1101);
    assert!(m.can_transfer(&alice, &Address::generate(&env), &1, &ta)); // lock elapsed on A
    assert_eq!(m.unlock_at(&ta, &alice), 1100);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn post_event_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.created(&Address::generate(&env), &1, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn set_duration_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.set_duration(&Address::generate(&env), &1);
}
```

- [ ] **Step 3: Run RED** — `cargo test -p constella-hub-module-lockup 2>&1 | tail -12`.

- [ ] **Step 4: Implement** — `src/lib.rs`
```rust
#![no_std]
//! Multi-tenant Lockup module: locks a holder's tokens for `duration(token)` seconds
//! from acquisition, per token. Shared instance; `Duration(token)` + `Acquired(token,
//! holder)` keyed by token; acquisition times recorded only from the hub's post-event
//! fan-out (hub-authed). Uses ledger time only — no balance mirror.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Duration(Address),          // token -> lock seconds
    Acquired(Address, Address), // (token, holder) -> ledger time
}

#[contract]
pub struct LockupHubModule;

#[contractimpl]
impl LockupHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn set_duration(env: Env, token: Address, secs: u64) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Duration(token), &secs);
    }
    pub fn unlock_at(env: Env, token: Address, holder: Address) -> u64 {
        match env.storage().persistent().get::<DataKey, u64>(&DataKey::Acquired(token.clone(), holder)) {
            Some(acq) => acq + Self::duration(&env, &token),
            None => 0,
        }
    }
    pub fn can_transfer(env: Env, from: Address, _to: Address, _amount: i128, token: Address) -> bool {
        match env.storage().persistent().get::<DataKey, u64>(&DataKey::Acquired(token.clone(), from)) {
            Some(acq) => env.ledger().timestamp() >= acq + Self::duration(&env, &token),
            None => true,
        }
    }
    pub fn can_create(_env: Env, _to: Address, _amount: i128, _token: Address) -> bool { true }
    pub fn transferred(env: Env, _from: Address, to: Address, _amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::record(&env, &token, &to);
    }
    pub fn created(env: Env, to: Address, _amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::record(&env, &token, &to);
    }
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl LockupHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn duration(env: &Env, token: &Address) -> u64 {
        env.storage().persistent().get(&DataKey::Duration(token.clone())).unwrap_or(0)
    }
    fn record(env: &Env, token: &Address, who: &Address) {
        let now = env.ledger().timestamp();
        env.storage().persistent().set(&DataKey::Acquired(token.clone(), who.clone()), &now);
    }
}
```
> NOTE: the `DataKey::Acquired(Address, Address),` line above uses ASCII — ensure the comma and parens are plain ASCII (no full-width characters). Write it as `Acquired(Address, Address), // (token, holder) -> ledger time`.

- [ ] **Step 5: Run GREEN + clippy + commit**
```bash
cargo test -p constella-hub-module-lockup 2>&1 | tail -6
cargo clippy -p constella-hub-module-lockup --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub-module-lockup
git commit -m "feat(hub): token-keyed Lockup module (per-token acquisition lock)"
```

---

### Task 3: `hub-module-transfer-window` — token-keyed config-only (TDD)

**Files:** Create `crates/hub-module-transfer-window/{Cargo.toml, src/lib.rs, src/test.rs}`

**Interfaces:** `TransferWindowHubModule` — `__constructor(hub)`; `pause(token)`/`unpause(token)`/`set_window(token, from: Option<u64>, until: Option<u64>)` (hub-authed); reads `is_paused(token)->bool`, `window(token)->(Option<u64>,Option<u64>)`; hooks `can_transfer`/`can_create` return `is_open(token)` (not paused AND within window); post-events no-op.

- [ ] **Step 1: Cargo.toml** (name `constella-hub-module-transfer-window`; deps soroban-sdk + dev testutils).

- [ ] **Step 2: Failing tests** — `src/test.rs`
```rust
#![cfg(test)]
use crate::{TransferWindowHubModule, TransferWindowHubModuleClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

fn setup(env: &Env) -> (TransferWindowHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(TransferWindowHubModule, (hub.clone(),));
    (TransferWindowHubModuleClient::new(env, &id), hub)
}

#[test]
fn pause_and_window_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    let x = Address::generate(&env);
    // both start open
    assert!(m.can_create(&x, &1, &ta) && m.can_create(&x, &1, &tb));
    m.pause(&ta);
    assert!(!m.can_create(&x, &1, &ta)); // A frozen
    assert!(m.can_create(&x, &1, &tb));  // B unaffected — isolated
    m.unpause(&ta);
    assert!(m.can_create(&x, &1, &ta));
    // window on A only
    m.set_window(&ta, &Some(100), &None);
    env.ledger().set_timestamp(50);
    assert!(!m.can_transfer(&x, &x, &1, &ta)); // before open_from on A
    assert!(m.can_transfer(&x, &x, &1, &tb));  // B has no window
    assert_eq!(m.is_paused(&ta), false);
    assert_eq!(m.window(&ta), (Some(100), None));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn pause_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.pause(&Address::generate(&env));
}
```

- [ ] **Step 3: Run RED** — `cargo test -p constella-hub-module-transfer-window 2>&1 | tail -12`.

- [ ] **Step 4: Implement** — `src/lib.rs`
```rust
#![no_std]
//! Multi-tenant TransferWindow module: admin freeze + time window, per token. Shared
//! instance; `Paused(token)` + `Window(token)` keyed by token. Reads only its config and
//! the ledger clock — no post-event bookkeeping.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Paused(Address),
    Window(Address),
}

#[contract]
pub struct TransferWindowHubModule;

#[contractimpl]
impl TransferWindowHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn pause(env: Env, token: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Paused(token), &true);
    }
    pub fn unpause(env: Env, token: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Paused(token), &false);
    }
    pub fn set_window(env: Env, token: Address, open_from: Option<u64>, open_until: Option<u64>) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Window(token), &(open_from, open_until));
    }
    pub fn is_paused(env: Env, token: Address) -> bool {
        env.storage().persistent().get(&DataKey::Paused(token)).unwrap_or(false)
    }
    pub fn window(env: Env, token: Address) -> (Option<u64>, Option<u64>) {
        env.storage().persistent().get(&DataKey::Window(token)).unwrap_or((None, None))
    }
    pub fn can_transfer(env: Env, _from: Address, _to: Address, _amount: i128, token: Address) -> bool {
        Self::is_open(&env, &token)
    }
    pub fn can_create(env: Env, _to: Address, _amount: i128, token: Address) -> bool {
        Self::is_open(&env, &token)
    }
    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl TransferWindowHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn is_open(env: &Env, token: &Address) -> bool {
        if Self::is_paused(env.clone(), token.clone()) { return false; }
        let (open_from, open_until) = Self::window(env.clone(), token.clone());
        let now = env.ledger().timestamp();
        if let Some(from) = open_from { if now < from { return false; } }
        if let Some(until) = open_until { if now > until { return false; } }
        true
    }
}
```

- [ ] **Step 5: Run GREEN + clippy + commit**
```bash
cargo test -p constella-hub-module-transfer-window 2>&1 | tail -6
cargo clippy -p constella-hub-module-transfer-window --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub-module-transfer-window
git commit -m "feat(hub): token-keyed TransferWindow module (per-token freeze + window)"
```

---

### Task 4: Extend `module-interface` with the 3 client traits

**Files:** Modify `crates/module-interface/src/lib.rs`

- [ ] **Step 1: Append the three traits** (beside the existing `CountryRestrictAdmin`)
```rust
/// Config surface of the multi-tenant MaxHolders module, called by the hub. Token-keyed.
#[contractclient(name = "MaxHoldersClient")]
pub trait MaxHoldersAdmin {
    fn set_max(env: Env, token: Address, cap: u32);
    fn max(env: Env, token: Address) -> u32;
    fn holders(env: Env, token: Address) -> u32;
}

/// Config surface of the multi-tenant Lockup module, called by the hub. Token-keyed.
#[contractclient(name = "LockupClient")]
pub trait LockupAdmin {
    fn set_duration(env: Env, token: Address, secs: u64);
    fn unlock_at(env: Env, token: Address, holder: Address) -> u64;
}

/// Config surface of the multi-tenant TransferWindow module, called by the hub. Token-keyed.
#[contractclient(name = "TransferWindowClient")]
pub trait TransferWindowAdmin {
    fn pause(env: Env, token: Address);
    fn unpause(env: Env, token: Address);
    fn set_window(env: Env, token: Address, open_from: Option<u64>, open_until: Option<u64>);
    fn is_paused(env: Env, token: Address) -> bool;
    fn window(env: Env, token: Address) -> (Option<u64>, Option<u64>);
}
```

- [ ] **Step 2: Build + commit**
```bash
cargo build -p constella-module-interface 2>&1 | tail -1
cargo clippy -p constella-module-interface --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/module-interface/src/lib.rs
git commit -m "feat(interface): MaxHolders/Lockup/TransferWindow clients for the hub"
```

---

### Task 5: Hub — wire all three into launch + forwarders + config (TDD)

**Files:** Modify `crates/hub/src/lib.rs`, `crates/hub/src/test.rs`

**Interfaces:** `LaunchConfig` gains `max_holders: u32`, `lockup: u64`, `transfer_window: bool`. `launch` registers each selected module + inits config. Forwarders: `set_max_holders(token, cap)` + reads `max_holders(token)`/`holders(token)`; `set_lockup(token, secs)` + read `unlock_at(token, holder)`; `pause(token)`/`unpause(token)`/`set_window(token, from, until)` + reads `is_paused(token)`/`transfer_window(token)`.

- [ ] **Step 1: Failing combined e2e test** — append to `crates/hub/src/test.rs`

Add `contractimport!` for the 3 module wasm. Then a test that launches a token with all three, and checks each enforces (holder cap; freeze blocks a mint; and a two-token isolation assertion for pause). Reference:
```rust
mod holders_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_max_holders.wasm"); }
mod lockup_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_lockup.wasm"); }
mod window_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_transfer_window.wasm"); }

#[test]
fn two_tokens_nonidentity_modules_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let holders = env.register(holders_wasm::WASM, (hub_addr.clone(),));
    let window = env.register(window_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "max_holders"), &holders);
    hub.set_module_addr(&Symbol::new(&env, "transfer_window"), &window);

    // token A: holder cap 1 + transfer_window; token B: just transfer_window
    let ta = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env], max_holders: 1, lockup: 0, transfer_window: true }).token;
    let tb = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env], max_holders: 0, lockup: 0, transfer_window: true }).token;
    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    tok_a.mint(&alice, &10);                 // 1st holder ok
    assert!(tok_a.try_mint(&bob, &10).is_err()); // A cap 1 -> 2nd holder denied
    // freeze A only
    hub.pause(&ta);
    assert!(tok_a.try_mint(&alice, &1).is_err()); // A frozen
    tok_b.mint(&alice, &1);                   // B not frozen -> ok (isolated)
    assert_eq!(hub.is_paused(&ta), true);
    assert_eq!(hub.is_paused(&tb), false);
}
```

- [ ] **Step 2: Run RED** — `stellar contract build >/dev/null && cargo test -p constella-hub two_tokens_nonidentity 2>&1 | tail -12`.

- [ ] **Step 3: Implement** — `crates/hub/src/lib.rs`

Add fields to `LaunchConfig`:
```rust
    pub max_holders: u32,      // 0 = not selected
    pub lockup: u64,           // 0 = not selected
    pub transfer_window: bool,
```
In `launch`, after the country_restrict block, add:
```rust
        if config.max_holders > 0 {
            let m = Self::module_addr(&env, "max_holders");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER, hooks::CREATED, hooks::TRANSFERRED, hooks::DESTROYED] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            MaxHoldersClient::new(&env, &m).set_max(&token, &config.max_holders);
        }
        if config.lockup > 0 {
            let m = Self::module_addr(&env, "lockup");
            for h in [hooks::CAN_TRANSFER, hooks::CREATED, hooks::TRANSFERRED] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            LockupClient::new(&env, &m).set_duration(&token, &config.lockup);
        }
        if config.transfer_window {
            let m = Self::module_addr(&env, "transfer_window");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
        }
```
Extend the `module-interface` `use` to include `MaxHoldersClient, LockupClient, TransferWindowClient`. Add forwarders + reads to the impl block:
```rust
    pub fn set_max_holders(env: Env, token: Address, cap: u32) {
        Self::require_token_admin(&env, &token);
        MaxHoldersClient::new(&env, &Self::module_addr(&env, "max_holders")).set_max(&token, &cap);
    }
    pub fn max_holders(env: Env, token: Address) -> u32 {
        MaxHoldersClient::new(&env, &Self::module_addr(&env, "max_holders")).max(&token)
    }
    pub fn holders(env: Env, token: Address) -> u32 {
        MaxHoldersClient::new(&env, &Self::module_addr(&env, "max_holders")).holders(&token)
    }
    pub fn set_lockup(env: Env, token: Address, secs: u64) {
        Self::require_token_admin(&env, &token);
        LockupClient::new(&env, &Self::module_addr(&env, "lockup")).set_duration(&token, &secs);
    }
    pub fn unlock_at(env: Env, token: Address, holder: Address) -> u64 {
        LockupClient::new(&env, &Self::module_addr(&env, "lockup")).unlock_at(&token, &holder)
    }
    pub fn pause(env: Env, token: Address) {
        Self::require_token_admin(&env, &token);
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).pause(&token);
    }
    pub fn unpause(env: Env, token: Address) {
        Self::require_token_admin(&env, &token);
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).unpause(&token);
    }
    pub fn set_window(env: Env, token: Address, open_from: Option<u64>, open_until: Option<u64>) {
        Self::require_token_admin(&env, &token);
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).set_window(&token, &open_from, &open_until);
    }
    pub fn is_paused(env: Env, token: Address) -> bool {
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).is_paused(&token)
    }
    pub fn transfer_window(env: Env, token: Address) -> (Option<u64>, Option<u64>) {
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).window(&token)
    }
```

- [ ] **Step 4: Run GREEN** — `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -8`.
Update ALL existing `LaunchConfig` literals in `test.rs` to add `max_holders: 0, lockup: 0, transfer_window: false`.

- [ ] **Step 5: Negative-auth test + fmt/clippy/README/commit**

Add one negative-auth forwarder test (e.g. `only_token_admin_can_pause` — launch a token with transfer_window, `env.set_auths(&[])`, `hub.pause(&t)`, `#[should_panic]`). Update `crates/hub/README.md` (list the 3 modules + config fields). Then:
```bash
cargo fmt -p constella-hub
cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub
git commit -m "feat(hub): wire MaxHolders + Lockup + TransferWindow + isolation e2e"
```

---

### Task 6: Workspace + testnet (controller spike)

- [ ] **Step 1: Workspace gate**
```bash
stellar contract build >/dev/null
cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"
cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
```

- [ ] **Step 2: Testnet (controller runs)** — deploy hub + the 3 shared modules; configure; a funded issuer launches ONE tx with `max_holders: 1, lockup: 0, transfer_window: true`; prove: a 2nd holder mint reverts (cap 1); `hub.pause(token)` then a mint reverts (frozen), `hub.unpause` then it passes. Record the one-signature launch tx + the enforced/reverted operations; append evidence to `crates/hub/README.md`.
