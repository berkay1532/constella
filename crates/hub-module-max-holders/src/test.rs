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
