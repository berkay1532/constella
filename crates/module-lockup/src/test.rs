#![cfg(test)]
//! Unit tests for the Lockup compliance module.

use crate::{LockupModule, LockupModuleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// A spoofer (arbitrary account, NOT the dispatcher) must not be able to drive a
// post-event directly. We do NOT mock the dispatcher's auth, so require_auth() on
// `dispatcher` must reject.
#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn transferred_rejects_non_dispatcher_caller() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let m = env.register(LockupModule, (admin, dispatcher, 3_600u64));
    let c = LockupModuleClient::new(&env, &m);
    let to = Address::generate(&env);
    let from = Address::generate(&env);
    // No auth for `dispatcher` -> require_dispatcher() must panic.
    c.transferred(&from, &to, &100, &Address::generate(&env));
}

// With the dispatcher's auth present (mocked here to stand in for the dispatcher
// contract calling), the post-event proceeds as before.
#[test]
fn transferred_proceeds_with_dispatcher_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let m = env.register(LockupModule, (admin, dispatcher, 3_600u64));
    let c = LockupModuleClient::new(&env, &m);
    let to = Address::generate(&env);
    let from = Address::generate(&env);
    c.transferred(&from, &to, &100, &Address::generate(&env));
    assert_eq!(c.unlock_at(&to), env.ledger().timestamp() + 3_600);
}
