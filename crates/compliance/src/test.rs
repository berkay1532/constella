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
