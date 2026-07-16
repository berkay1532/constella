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
