# Multi-tenant Hub — add MaxInvestorsPerCountry (completes 7/7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the last module — MaxInvestorsPerCountry — to the multi-tenant hub, completing all 7. It combines two already-proven mechanics: the per-token balance mirror (max-balance) and the per-token identity (country-restrict), plus a per-(token,country) holder count.

**Architecture:** A shared, token-keyed module keys `Cap(token)`, `Count(token,country)`, `Bal(token,holder)`, `Identity(token)`; it reads `country_of` from the token's identity and updates the per-country count from the hub's post-event fan-out (hub-authed). The hub deploys **one** identity per token (shared with country-restrict if both are selected — the launch identity deploy is hoisted so a token has a single identity instance), registers max-investors on all 5 hooks, and configures it with that identity + cap.

**Tech Stack:** Rust / soroban-sdk 26, stellar CLI (testnet).

## Global Constraints

- English; builds to `wasm32v1-none`; `cargo test --workspace` + `cargo clippy` green (no bool-literal `assert_eq!`). TDD (RED before GREEN).
- **Per-token isolation:** every stateful key carries `token`; a token gets ONE identity instance even if it selects both country-restrict and max-investors. Every test uses two tokens.
- Hub must NOT depend on any `#[contract]` crate; use `#[contractclient]` traits (`ModuleClient` for hooks, a new `MaxInvestorsClient` for config). The module reads `country_of` via the existing `IdentityClient`.
- Module mutators + post-events require `hub.require_auth()`; forwarders require the per-token issuer. Guard tests pinned to `#[should_panic(expected = "Error(Auth, InvalidAction)")]`.
- Reuse the hub's `deploy`/`register`/`module_addr`/`require_token_admin`/`hooks` helpers.
- Sentinel: `LaunchConfig.max_investors: u32` (0 = not selected).
- Hub tests `contractimport!` module + identity wasm → run `stellar contract build` before `cargo test -p constella-hub`.

---

### Task 1: `hub-module-max-investors-per-country` — token-keyed identity + mirror (TDD)

**Files:** Create `crates/hub-module-max-investors-per-country/{Cargo.toml, src/lib.rs, src/test.rs}`

**Interfaces:** `MaxInvestorsHubModule` — `__constructor(hub)`; `configure(token, identity, cap: u32)` + `set_cap(token, cap: u32)` (hub-authed); reads `cap(token)->u32`, `count(token,country)->u32`; hooks `can_transfer`/`can_create` (per-country cap with net-zero); post-events (hub-authed) maintain `Bal(token,holder)` + `Count(token,country)` via holder transitions.

- [ ] **Step 1: Cargo.toml** (name `constella-hub-module-max-investors-per-country`; deps `soroban-sdk` + `constella-module-interface`; dev-deps testutils + `constella-identity-mock`).

- [ ] **Step 2: Failing tests** — `src/test.rs`
```rust
#![cfg(test)]
use crate::{MaxInvestorsHubModule, MaxInvestorsHubModuleClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const US: u32 = 840;
const DE: u32 = 276;

fn setup(env: &Env) -> (MaxInvestorsHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(MaxInvestorsHubModule, (hub.clone(),));
    (MaxInvestorsHubModuleClient::new(env, &id), hub)
}

#[test]
fn count_and_cap_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);
    let id_a = env.register(IdentityMock, (admin.clone(),));
    let id_b = env.register(IdentityMock, (admin.clone(),));
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.configure(&ta, &id_a, &2); // token A cap 2/country
    m.configure(&tb, &id_b, &1); // token B cap 1/country
    let ida = IdentityMockClient::new(&env, &id_a);
    let idb = IdentityMockClient::new(&env, &id_b);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    ida.set_country(&a, &US);
    ida.set_country(&b, &US);
    idb.set_country(&a, &US);

    m.created(&a, &100, &ta);
    m.created(&b, &100, &ta);
    assert_eq!(m.count(&ta, &US), 2);
    let c = Address::generate(&env);
    ida.set_country(&c, &US);
    assert!(!m.can_create(&c, &1, &ta)); // US full at 2 on token A
    // token B independent: its US count is 0
    assert!(m.can_create(&a, &1, &tb));
    // unattested recipient denied
    assert!(!m.can_create(&Address::generate(&env), &1, &ta));
    let _ = (DE,);
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
fn configure_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.configure(&Address::generate(&env), &Address::generate(&env), &1);
}
```

- [ ] **Step 3: Run RED** — `cargo test -p constella-hub-module-max-investors-per-country 2>&1 | tail -12` (compile fail).

- [ ] **Step 4: Implement** — `src/lib.rs`
```rust
#![no_std]
//! Multi-tenant MaxInvestorsPerCountry module: caps the number of distinct holders
//! attributed to any single country, per token. Combines a per-token balance mirror
//! (to detect holder transitions) with the token's per-token identity provider (to
//! bucket holders by country). All state keyed by (token, …); mutated only from the
//! hub's post-event fan-out (hub-authed).

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Identity(Address),     // token -> identity provider
    Cap(Address),          // token -> per-country cap
    Count(Address, u32),   // (token, country) -> distinct holders
    Bal(Address, Address), // (token, holder) -> balance
}

#[contract]
pub struct MaxInvestorsHubModule;

#[contractimpl]
impl MaxInvestorsHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn configure(env: Env, token: Address, identity: Address, cap: u32) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Identity(token.clone()), &identity);
        env.storage().persistent().set(&DataKey::Cap(token), &cap);
    }
    pub fn set_cap(env: Env, token: Address, cap: u32) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Cap(token), &cap);
    }
    pub fn cap(env: Env, token: Address) -> u32 {
        env.storage().persistent().get(&DataKey::Cap(token)).unwrap_or(0)
    }
    pub fn count(env: Env, token: Address, country: u32) -> u32 {
        env.storage().persistent().get(&DataKey::Count(token, country)).unwrap_or(0)
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, amount: i128, token: Address) -> bool {
        if amount <= 0 { return true; }
        let country = match Self::country_of(&env, &token, &to) { Some(c) => c, None => return false };
        if Self::bal(&env, &token, &to) > 0 { return true; }
        let from_frees_slot = Self::bal(&env, &token, &from) > 0
            && Self::bal(&env, &token, &from) - amount == 0
            && Self::country_of(&env, &token, &from) == Some(country);
        if from_frees_slot { return true; }
        Self::count(env.clone(), token.clone(), country) < Self::cap(env, token)
    }

    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        if amount <= 0 { return true; }
        let country = match Self::country_of(&env, &token, &to) { Some(c) => c, None => return false };
        if Self::bal(&env, &token, &to) > 0 { return true; }
        Self::count(env.clone(), token.clone(), country) < Self::cap(env, token)
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

impl MaxInvestorsHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn bal(env: &Env, token: &Address, who: &Address) -> i128 {
        env.storage().persistent().get(&DataKey::Bal(token.clone(), who.clone())).unwrap_or(0)
    }
    fn country_of(env: &Env, token: &Address, who: &Address) -> Option<u32> {
        let identity: Address = env.storage().persistent().get(&DataKey::Identity(token.clone())).unwrap();
        IdentityClient::new(env, &identity).country_of(who)
    }
    fn bump_count(env: &Env, token: &Address, country: u32, delta: i32) {
        let key = DataKey::Count(token.clone(), country);
        let cur: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        let next = if delta < 0 { cur.saturating_sub((-delta) as u32) } else { cur + delta as u32 };
        env.storage().persistent().set(&key, &next);
    }
    fn apply(env: &Env, token: &Address, who: &Address, delta: i128) {
        if delta == 0 { return; }
        let key = DataKey::Bal(token.clone(), who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new = old + delta;
        env.storage().persistent().set(&key, &new);
        if old <= 0 && new > 0 {
            if let Some(c) = Self::country_of(env, token, who) { Self::bump_count(env, token, c, 1); }
        } else if old > 0 && new <= 0 {
            if let Some(c) = Self::country_of(env, token, who) { Self::bump_count(env, token, c, -1); }
        }
    }
}
```

- [ ] **Step 5: Run GREEN + clippy + commit**
```bash
cargo test -p constella-hub-module-max-investors-per-country 2>&1 | tail -6
cargo clippy -p constella-hub-module-max-investors-per-country --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub-module-max-investors-per-country
git commit -m "feat(hub): token-keyed MaxInvestorsPerCountry module (mirror + per-token identity)"
```

---

### Task 2: Extend `module-interface` with `MaxInvestorsClient`

**Files:** Modify `crates/module-interface/src/lib.rs`

- [ ] **Step 1: Append the trait** (beside the others)
```rust
/// Config surface of the multi-tenant MaxInvestorsPerCountry module, called by the hub. Token-keyed.
#[contractclient(name = "MaxInvestorsClient")]
pub trait MaxInvestorsAdmin {
    fn configure(env: Env, token: Address, identity: Address, cap: u32);
    fn set_cap(env: Env, token: Address, cap: u32);
    fn cap(env: Env, token: Address) -> u32;
    fn count(env: Env, token: Address, country: u32) -> u32;
}
```

- [ ] **Step 2: Build + commit**
```bash
cargo build -p constella-module-interface 2>&1 | tail -1
cargo clippy -p constella-module-interface --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/module-interface/src/lib.rs
git commit -m "feat(interface): MaxInvestorsClient for the hub's last module"
```

---

### Task 3: Hub — hoist the per-token identity + wire MaxInvestors (TDD)

**Files:** Modify `crates/hub/src/lib.rs`, `crates/hub/src/test.rs`

**Interfaces:** `LaunchConfig.max_investors: u32` (0 = off). `launch` deploys the per-token identity ONCE if any identity-dependent module is selected (country_restrict OR max_investors), then both modules read that stored `Identity(token)`. max_investors registered on all 5 hooks + `configure(token, identity, cap)`. Forwarder `set_investor_cap(token, cap)` + reads `investor_cap(token)`, `investor_count(token, country)`.

- [ ] **Step 1: Failing e2e test** — append to `crates/hub/src/test.rs`

Add `contractimport!` for the max-investors wasm (identity wasm import already exists from the country-restrict slice). Then:
```rust
mod investors_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_max_investors_per_country.wasm"); }

#[test]
fn two_tokens_max_investors_isolated_and_shares_identity() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let identity_hash: BytesN<32> = env.deployer().upload_contract_wasm(identity_wasm::WASM);
    hub.set_identity_wasm(&identity_hash);
    let investors = env.register(investors_wasm::WASM, (hub_addr.clone(),));
    let country = env.register(country_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "max_investors"), &investors);
    hub.set_module_addr(&Symbol::new(&env, "country_restrict"), &country);

    // token A: country_restrict [US] + max_investors cap 1 (both identity-dependent -> ONE identity)
    let ta = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env, 840u32], max_holders: 0, lockup: 0, transfer_window: false, max_investors: 1 }).token;
    // token B: max_investors cap 2 only
    let tb = hub.launch(&LaunchConfig { admin: Address::generate(&env), denylist: false, max_balance: 0, country_restrict: soroban_sdk::vec![&env], max_holders: 0, lockup: 0, transfer_window: false, max_investors: 2 }).token;

    // token A shares ONE identity across country_restrict + max_investors
    let id_a = identity_wasm::Client::new(&env, &hub.identity(&ta));
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    id_a.set_country(&alice, &840);
    id_a.set_country(&bob, &840);
    let tok_a = token_wasm::Client::new(&env, &ta);
    tok_a.mint(&alice, &10);                 // US holder 1 (cap 1)
    assert!(tok_a.try_mint(&bob, &10).is_err()); // US full at 1 on token A
    assert_eq!(hub.investor_count(&ta, &840), 1);
    // token B independent (cap 2, its own identity)
    let id_b = identity_wasm::Client::new(&env, &hub.identity(&tb));
    id_b.set_country(&alice, &840);
    let tok_b = token_wasm::Client::new(&env, &tb);
    tok_b.mint(&alice, &10);                  // ok on B
    assert_eq!(hub.investor_count(&tb, &840), 1);
}
```

- [ ] **Step 2: Run RED** — `stellar contract build >/dev/null && cargo test -p constella-hub two_tokens_max_investors 2>&1 | tail -12`.

- [ ] **Step 3: Implement — hoist identity + wire max_investors** — `crates/hub/src/lib.rs`

Add `max_investors: u32` to `LaunchConfig`.

**Refactor the identity deploy out of the country_restrict block.** Replace the current country_restrict block (which deploys the identity inline) with: a hoisted deploy, then a country_restrict block that READS the stored identity, then a max_investors block. Insert (after the max_balance block, replacing the existing `if !config.country_restrict.is_empty()` block):
```rust
        // Deploy ONE identity per token if any identity-dependent module is selected,
        // so country_restrict and max_investors share it.
        if !config.country_restrict.is_empty() || config.max_investors > 0 {
            let identity_hash: BytesN<32> = env.storage().instance().get(&DataKey::IdentityWasm).unwrap();
            let identity = Self::deploy(&env, &identity_hash, (config.admin.clone(),));
            env.storage().persistent().set(&DataKey::Identity(token.clone()), &identity);
        }
        if !config.country_restrict.is_empty() {
            let identity: Address = env.storage().persistent().get(&DataKey::Identity(token.clone())).unwrap();
            let m = Self::module_addr(&env, "country_restrict");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            CountryRestrictClient::new(&env, &m).configure(&token, &identity, &config.country_restrict);
        }
        if config.max_investors > 0 {
            let identity: Address = env.storage().persistent().get(&DataKey::Identity(token.clone())).unwrap();
            let m = Self::module_addr(&env, "max_investors");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER, hooks::CREATED, hooks::TRANSFERRED, hooks::DESTROYED] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            MaxInvestorsClient::new(&env, &m).configure(&token, &identity, &config.max_investors);
        }
```
Extend the `module-interface` `use` to include `MaxInvestorsClient`. Add forwarder + reads:
```rust
    pub fn set_investor_cap(env: Env, token: Address, cap: u32) {
        Self::require_token_admin(&env, &token);
        MaxInvestorsClient::new(&env, &Self::module_addr(&env, "max_investors")).set_cap(&token, &cap);
    }
    pub fn investor_cap(env: Env, token: Address) -> u32 {
        MaxInvestorsClient::new(&env, &Self::module_addr(&env, "max_investors")).cap(&token)
    }
    pub fn investor_count(env: Env, token: Address, country: u32) -> u32 {
        MaxInvestorsClient::new(&env, &Self::module_addr(&env, "max_investors")).count(&token, &country)
    }
```

- [ ] **Step 4: Run GREEN** — `stellar contract build >/dev/null && cargo test -p constella-hub 2>&1 | tail -8`.
Update ALL existing `LaunchConfig` literals in `test.rs` to add `max_investors: 0`. The existing country_restrict e2e must still pass (the hoist is behavior-preserving for the country-restrict-only case — the identity is now deployed in the hoisted block, then read).

- [ ] **Step 5: Negative-auth test + fmt/clippy/README/commit**

Add `only_token_admin_can_set_investor_cap` (`env.set_auths(&[])` → `hub.set_investor_cap` → `#[should_panic]`). Update `crates/hub/README.md` (max-investors + the shared-identity note). Then fmt/clippy (clean)/commit:
```bash
cargo fmt -p constella-hub
cargo clippy -p constella-hub --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/hub
git commit -m "feat(hub): wire MaxInvestorsPerCountry + hoist shared per-token identity (7/7)"
```

---

### Task 4: Workspace + testnet (controller spike)

- [ ] **Step 1: Workspace gate**
```bash
stellar contract build >/dev/null
cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"
cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
```

- [ ] **Step 2: Testnet (controller runs)** — deploy hub + shared max-investors module; configure; a funded issuer launches ONE tx with `max_investors: 1`. Read `hub.identity(token)`, attest two accounts as the same country on it, mint the first (passes, count=1) and the second (reverts — country at cap 1). Record the one-signature launch tx + the count + the enforced revert; append evidence to `crates/hub/README.md`. This completes 7/7 hub modules.
