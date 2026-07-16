#![cfg(test)]
use crate::{DenylistHubModule, DenylistHubModuleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (DenylistHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(DenylistHubModule, (hub.clone(),));
    (DenylistHubModuleClient::new(env, &id), hub)
}

#[test]
fn denylist_is_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let alice = Address::generate(&env);
    let from = Address::generate(&env);

    m.add_to_denylist(&token_a, &alice);
    // token_a sees alice denied; token_b does NOT.
    assert!(m.is_denied(&token_a, &alice));
    assert!(!m.is_denied(&token_b, &alice));
    assert!(!m.can_transfer(&from, &alice, &1, &token_a)); // blocked on A
    assert!(m.can_transfer(&from, &alice, &1, &token_b));  // allowed on B

    m.remove_from_denylist(&token_a, &alice);
    assert!(!m.is_denied(&token_a, &alice));
    assert!(m.can_transfer(&from, &alice, &1, &token_a));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn mutators_require_hub_auth() {
    let env = Env::default();
    // no mock_all_auths -> hub.require_auth() must reject.
    let (m, _hub) = setup(&env);
    m.add_to_denylist(&Address::generate(&env), &Address::generate(&env));
}
