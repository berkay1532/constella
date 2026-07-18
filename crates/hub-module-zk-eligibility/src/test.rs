#![cfg(test)]
use crate::{ZkEligibilityHubModule, ZkEligibilityHubModuleClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (ZkEligibilityHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(ZkEligibilityHubModule, (hub.clone(),));
    (ZkEligibilityHubModuleClient::new(env, &id), hub)
}

// Each token has its own ZK identity; eligibility is read from is_verified and is isolated
// per (token, identity). identity-mock's `set_verified` stands in for a real ZK identity's
// post-proof flag.
#[test]
fn gates_on_is_verified_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);
    let id_a = env.register(IdentityMock, (admin.clone(),));
    let id_b = env.register(IdentityMock, (admin.clone(),));
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.configure(&ta, &id_a);
    m.configure(&tb, &id_b);

    let alice = Address::generate(&env);
    // alice is verified on token A's identity only.
    IdentityMockClient::new(&env, &id_a).set_verified(&alice, &true);

    assert!(m.can_create(&alice, &1, &ta)); // verified on A -> allowed
    assert!(m.can_transfer(&Address::generate(&env), &alice, &1, &ta));
    assert!(!m.can_create(&alice, &1, &tb)); // not verified on B -> denied (isolated)
    // an unverified recipient on A is denied
    assert!(!m.can_create(&Address::generate(&env), &1, &ta));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn configure_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.configure(&Address::generate(&env), &Address::generate(&env));
}

#[test]
fn identity_read_returns_configured() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let t = Address::generate(&env);
    let id = Address::generate(&env);
    m.configure(&t, &id);
    assert_eq!(m.identity(&t), id);
}
