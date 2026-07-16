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
