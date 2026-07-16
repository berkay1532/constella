# v1 Post-Event Auth Patch (Phase 0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the live v1 vulnerability where anyone can corrupt the stateful compliance modules' mirrors by calling unauthenticated post-events, letting them bypass caps.

**Architecture:** Authenticate the post-event chain by the *immediate caller* at each hop. The dispatcher's post-events require the token's auth (`token.require_auth()` — the token is the dispatcher's direct caller). Each stateful module stores its dispatcher address at construction and requires the dispatcher's auth in its state-mutating post-events. Both auto-satisfy in the real `token → dispatcher → module` flow (each targets the direct caller) and reject spoofers (who cannot forge a contract's authorization).

**Tech Stack:** Rust / soroban-sdk 26 (`Address::require_auth`), stellar CLI (deploy script).

## Global Constraints

- Everything committed is in English (code, comments, commits).
- Contracts build to `wasm32v1-none`; `cargo test --workspace` and `cargo clippy` stay green.
- TDD (RED before GREEN). Security tests must assert the *property* (spoof rejected, legit flow works) — do NOT hide it behind blanket `mock_all_auths()` on the guarded call.
- This is a security patch to v1 — changing the 4 stateful modules' constructors (adding a `dispatcher` param) and the deploy/demo wiring is in scope and expected.
- Only the 4 modules that mutate state in post-events are guarded: `module-max-holders`, `module-max-balance`, `module-max-investors-per-country`, `module-lockup`. The no-op-post-event modules (denylist, country-restrict, transfer-window) are unchanged.
- Auth mechanism (soroban-sdk 26): `X.require_auth()` for a contract address X auto-passes when X is the *immediate* contract caller of the current function; a non-X caller must present X's authorization, which only X (or a signer of X's auth entry) can. If a legit-flow test unexpectedly fails, the fallback is for the caller to `env.authorize_as_current_contract(...)` its sub-call — but verify whether that is needed before adding it (the direct-caller case should not need it).

---

### Task 1: Dispatcher post-events require the token's auth (TDD)

**Files:**
- Modify: `crates/compliance/src/lib.rs` (the 3 post-event fns, lines ~98-117)
- Test: `crates/compliance/src/test.rs` (new file; add `#[cfg(test)] mod test;` to lib.rs)

**Interfaces:**
- Produces: `Compliance::transferred/created/destroyed` now begin with `token.require_auth()`.

- [ ] **Step 1: Write the failing security test** — create `crates/compliance/src/test.rs`

```rust
#![cfg(test)]
use crate::{Compliance, ComplianceClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// A spoofer (arbitrary account, NOT the token contract) must not be able to drive a
// post-event. We do NOT mock the token's auth, so require_auth() on `token` must reject.
#[test]
#[should_panic]
fn transferred_rejects_caller_without_token_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let compliance = env.register(Compliance, (admin,));
    let c = ComplianceClient::new(&env, &compliance);

    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env); // some token address the caller does NOT control
    // No mock_all_auths / no auth for `token` -> require_auth(token) must panic.
    c.transferred(&from, &to, &100, &token);
}

// With the token's auth present (mocked here to stand in for the token contract calling),
// the post-event proceeds (no modules registered -> just returns).
#[test]
fn transferred_proceeds_with_token_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let compliance = env.register(Compliance, (admin,));
    let c = ComplianceClient::new(&env, &compliance);
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env);
    c.transferred(&from, &to, &100, &token); // must not panic
}
```

Add `#[cfg(test)] mod test;` near the top of `crates/compliance/src/lib.rs` (after the `use` lines).

- [ ] **Step 2: Run to verify the first test fails**

Run: `cargo test -p constella-compliance transferred_rejects 2>&1 | tail -12`
Expected: FAIL — `transferred_rejects_caller_without_token_auth` does NOT panic yet (no auth check), so `#[should_panic]` fails.

- [ ] **Step 3: Add `token.require_auth()` to the 3 post-events** — `crates/compliance/src/lib.rs`

At the very start of each of `transferred`, `created`, `destroyed`, add the auth gate. Full new bodies:

```rust
    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address) {
        token.require_auth();
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::Transferred);
        for m in mods.iter() {
            ModuleClient::new(&env, &m).transferred(&from, &to, &amount, &token);
        }
    }

    pub fn created(env: Env, to: Address, amount: i128, token: Address) {
        token.require_auth();
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::Created);
        for m in mods.iter() {
            ModuleClient::new(&env, &m).created(&to, &amount, &token);
        }
    }

    pub fn destroyed(env: Env, from: Address, amount: i128, token: Address) {
        token.require_auth();
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::Destroyed);
        for m in mods.iter() {
            ModuleClient::new(&env, &m).destroyed(&from, &amount, &token);
        }
    }
```

- [ ] **Step 4: Run to verify both tests pass**

Run: `cargo test -p constella-compliance 2>&1 | tail -8`
Expected: PASS — `transferred_rejects_caller_without_token_auth` now panics (rejected), `transferred_proceeds_with_token_auth` passes.

- [ ] **Step 5: clippy + commit**

```bash
cargo clippy -p constella-compliance --all-targets 2>&1 | grep -E "warning|error" | head
git add crates/compliance/src/lib.rs crates/compliance/src/test.rs
git commit -m "fix(compliance): post-events require the token's auth (block spoofed events)"
```

---

### Task 2: Stateful modules require the dispatcher's auth in post-events (TDD)

**Files:**
- Modify: `crates/module-max-holders/src/lib.rs`, `crates/module-max-balance/src/lib.rs`, `crates/module-max-investors-per-country/src/lib.rs`, `crates/module-lockup/src/lib.rs`
- Test: add a `#[should_panic]` guard test to each module's existing `test.rs` (max-holders/max-balance/lockup have inline tests; max-investors has `src/test.rs`). Where a module has no test file, add `#[cfg(test)] mod test;` + `src/test.rs`.

**Interfaces:**
- Consumes: nothing from Task 1 (independent contracts).
- Produces: each of the 4 modules gains a `dispatcher: Address` constructor param (stored under a new `DataKey::Dispatcher`) and calls `Self::require_dispatcher(&env)` at the start of every state-mutating post-event. New constructor signatures:
  - MaxHolders: `__constructor(env, admin, dispatcher, max: u32)`
  - MaxBalance: `__constructor(env, admin, dispatcher, max_per_holder: i128)`
  - MaxInvestorsPerCountry: `__constructor(env, admin, dispatcher, identity, cap: u32)`
  - Lockup: `__constructor(env, admin, dispatcher, lock_seconds: u64)`

**The uniform pattern (apply to each module):**
1. Add `Dispatcher` to the `DataKey` enum.
2. Add `dispatcher: Address` to `__constructor` (place it right after `admin`) and store it: `env.storage().instance().set(&DataKey::Dispatcher, &dispatcher);`.
3. Add a private helper:
   ```rust
       fn require_dispatcher(env: &Env) {
           let d: Address = env.storage().instance().get(&DataKey::Dispatcher).unwrap();
           d.require_auth();
       }
   ```
4. Call `Self::require_dispatcher(&env);` as the FIRST line of each state-mutating post-event.

- [ ] **Step 1: Write the failing guard test for MaxHolders** — append to `crates/module-max-holders/src/lib.rs`'s test module (it has an inline `#[cfg(test)] mod test` — if inline, add there; the crate keeps tests inline)

First confirm where MaxHolders' tests live: `grep -n "mod test\|#\[test\]" crates/module-max-holders/src/lib.rs crates/module-max-holders/src/test.rs 2>/dev/null`. Add this test in that location (create `src/test.rs` + `#[cfg(test)] mod test;` if none exists):

```rust
#[test]
#[should_panic]
fn created_rejects_non_dispatcher_caller() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let m = env.register(MaxHoldersModule, (admin, dispatcher, 5u32));
    let c = MaxHoldersModuleClient::new(&env, &m);
    let who = Address::generate(&env);
    // No auth for `dispatcher` -> require_dispatcher() must panic.
    c.created(&who, &100, &Address::generate(&env));
}
```
(Import `MaxHoldersModuleClient`, `Address`, `Env`, `testutils::Address as _` as the module's other tests do.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p constella-module-max-holders created_rejects 2>&1 | tail -12`
Expected: FAIL to COMPILE first — the 3-arg constructor `(admin, dispatcher, 5u32)` doesn't match the current 2-arg `(admin, max)`. That compile failure IS the RED signal that the constructor must change. (After Step 3 it compiles and the `#[should_panic]` passes.)

- [ ] **Step 3: Apply the pattern to MaxHolders** — `crates/module-max-holders/src/lib.rs`

Change the `DataKey` enum and constructor, and guard the 3 post-events:
```rust
#[contracttype]
enum DataKey {
    Admin,
    Dispatcher,
    Max,
    Count,
    Bal(Address),
}
```
```rust
    pub fn __constructor(env: Env, admin: Address, dispatcher: Address, max: u32) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Dispatcher, &dispatcher);
        env.storage().instance().set(&DataKey::Max, &max);
        env.storage().instance().set(&DataKey::Count, &0u32);
    }
```
Add `Self::require_dispatcher(&env);` as the first line of `transferred`, `created`, `destroyed`. Add the `require_dispatcher` helper (pattern above) to the private `impl` block.
Then update the module's existing happy-path tests to construct with the new 3-arg signature `(admin, dispatcher, max)` and, since they call post-events directly, wrap those calls with `env.mock_all_auths()` (they already do) so the dispatcher auth is satisfied.

- [ ] **Step 4: Run MaxHolders tests green**

Run: `cargo test -p constella-module-max-holders 2>&1 | tail -8`
Expected: PASS — guard test panics as expected; existing tests pass with `mock_all_auths` covering the dispatcher.

- [ ] **Step 5: Apply the identical pattern to the other 3 modules**

Repeat Steps 1–4 for each, using its constructor signature and guarding the listed post-events:
- **MaxBalance** (`crates/module-max-balance/src/lib.rs`): constructor `(admin, dispatcher, max_per_holder: i128)`; guard `transferred`, `created`, `destroyed`.
- **MaxInvestorsPerCountry** (`crates/module-max-investors-per-country/src/lib.rs`): constructor `(admin, dispatcher, identity, cap: u32)` (dispatcher right after admin); guard `transferred`, `created`, `destroyed`.
- **Lockup** (`crates/module-lockup/src/lib.rs`): constructor `(admin, dispatcher, lock_seconds: u64)`; guard `transferred`, `created` (its `destroyed` is a no-op — leave it unguarded, nothing to corrupt). Add the `Dispatcher` DataKey + `require_dispatcher` helper.

For each, add its own `created_rejects_non_dispatcher_caller` (or `transferred_rejects...` for lockup) `#[should_panic]` test, update existing tests to the new constructor arity, and run `cargo test -p <crate>` green.

- [ ] **Step 6: clippy + commit**

```bash
for c in max-holders max-balance max-investors-per-country lockup; do cargo clippy -p constella-module-$c --all-targets 2>&1 | grep -E "warning|error"; done | head
git add crates/module-max-holders crates/module-max-balance crates/module-max-investors-per-country crates/module-lockup
git commit -m "fix(modules): stateful post-events require the dispatcher's auth (block direct spoof)"
```

---

### Task 3: Rewire the deploy script + demo-token integration test; workspace green

**Files:**
- Modify: `scripts/deploy-testnet.sh` (constructors of the 4 modules)
- Modify: `crates/demo-token/src/test.rs` (the 4 modules' `env.register` calls + a new spoof assertion)

**Interfaces:**
- Consumes: the new 3/4/5-arg constructors from Task 2.

- [ ] **Step 1: Update the deploy script** — `scripts/deploy-testnet.sh`

The dispatcher (`$COMPLIANCE`) is deployed before the modules, so its address is available. Change the 4 module deploys to pass it right after `--admin`:
```bash
MAX_HOLDERS=$(dep constella_module_max_holders --admin "$ADMIN" --dispatcher "$COMPLIANCE" --max 5)
LOCKUP=$(dep constella_module_lockup --admin "$ADMIN" --dispatcher "$COMPLIANCE" --lock_seconds 0)
MAX_BALANCE=$(dep constella_module_max_balance --admin "$ADMIN" --dispatcher "$COMPLIANCE" --max_per_holder 1000000)
```
And in the reference-token section, `INVESTORS`:
```bash
INVESTORS=$(dep constella_module_max_investors_per_country --admin "$ADMIN" --dispatcher "$REF_COMPLIANCE" --identity "$IDENTITY" --cap 1)
```
(Use each module's own dispatcher: main-stack modules get `$COMPLIANCE`; reference-token modules get `$REF_COMPLIANCE`.) Do not re-run the deploy here — that is a separate step the user runs; just make the script correct.

- [ ] **Step 2: Update the demo-token integration test constructors** — `crates/demo-token/src/test.rs`

Both `full_compliance_flow` and `new_modules_compliance_flow` register these modules. Update each `env.register` to pass the compliance dispatcher (already in scope as `compliance` / `compliance` respectively) after `admin`:
```rust
let max_holders = env.register(MaxHoldersModule, (admin.clone(), compliance.clone(), 3u32));
let lockup = env.register(LockupModule, (admin.clone(), compliance.clone(), 100u64));
let max_balance = env.register(MaxBalanceModule, (admin.clone(), compliance.clone(), 1000i128));
// and in new_modules_compliance_flow:
let investors = env.register(MaxInvestorsPerCountryModule, (admin.clone(), compliance.clone(), identity.clone(), 2u32));
```
The tests already call `env.mock_all_auths()`, which covers the new dispatcher auth in the legit flow — so they keep passing.

- [ ] **Step 3: Add a spoof-rejection assertion to the integration test** — append inside `full_compliance_flow` (after the stack is wired), proving the end-to-end guard:

```rust
    // A spoofed post-event straight to the dispatcher (not from the token) is rejected.
    // Re-run without blanket auth for the token by using a targeted check: calling the
    // module's post-event directly from the test as a non-dispatcher must panic.
    // (Covered by unit tests; here we assert the dispatcher guard end-to-end.)
    // NOTE: full_compliance_flow uses mock_all_auths, so this assertion lives in the
    // module unit tests (Task 2). Leave a comment pointer here instead of a duplicate.
```
(If a runnable end-to-end spoof assertion is awkward under `mock_all_auths`, keep the security assertions in the Task-2 unit tests and add only this comment pointer — do not weaken `mock_all_auths` for the whole flow.)

- [ ] **Step 4: Full workspace gate**

Run: `cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"`
Run: `cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1`
Run: `bash -n scripts/deploy-testnet.sh && echo "deploy script syntax ok"`
Expected: workspace green; wasm builds; deploy script parses.

- [ ] **Step 5: Commit**

```bash
git add scripts/deploy-testnet.sh crates/demo-token/src/test.rs
git commit -m "fix(demo): wire dispatcher into stateful modules; deploy + integration updated"
```

---

## Notes for the reviewer / final gate

- The security property is: a post-event from anyone other than the correct immediate caller panics; the real `token → dispatcher → module` flow is unaffected. Verify the `#[should_panic]` tests do NOT rely on `mock_all_auths` for the guarded call (that would hide the property).
- This changes v1 module ABIs (constructor arity). That is intentional for a security patch; the README/docs contrast of v1-vs-v2 (separate doc task) should note the post-event auth requirement.
- If any legit-flow test fails because a contract's `require_auth()` for its direct caller does not auto-satisfy in soroban-sdk 26, the caller must `env.authorize_as_current_contract` its sub-call — but confirm this is actually needed before adding it (direct-caller auth is expected to pass).
