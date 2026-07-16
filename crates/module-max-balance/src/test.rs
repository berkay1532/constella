#![cfg(test)]
//! Unit tests for the MaxBalance compliance module.

use crate::{MaxBalanceModule, MaxBalanceModuleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// A spoofer (arbitrary account, NOT the dispatcher) must not be able to drive a
// post-event directly. We do NOT mock the dispatcher's auth, so require_auth() on
// `dispatcher` must reject.
#[test]
#[should_panic]
fn created_rejects_non_dispatcher_caller() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let m = env.register(MaxBalanceModule, (admin, dispatcher, 1_000i128));
    let c = MaxBalanceModuleClient::new(&env, &m);
    let who = Address::generate(&env);
    // No auth for `dispatcher` -> require_dispatcher() must panic.
    c.created(&who, &100, &Address::generate(&env));
}

// With the dispatcher's auth present (mocked here to stand in for the dispatcher
// contract calling), the post-event proceeds as before.
#[test]
fn created_proceeds_with_dispatcher_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let m = env.register(MaxBalanceModule, (admin, dispatcher, 1_000i128));
    let c = MaxBalanceModuleClient::new(&env, &m);
    let who = Address::generate(&env);
    c.created(&who, &950, &Address::generate(&env));
    // who's mirrored balance is now 950; topping up by 100 would exceed the 1000 cap.
    assert!(!c.can_create(&who, &100, &Address::generate(&env)));
}
