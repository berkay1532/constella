#![cfg(test)]
use crate::{CountryRestrictHubModule, CountryRestrictHubModuleClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

fn setup(env: &Env) -> (CountryRestrictHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(CountryRestrictHubModule, (hub.clone(),));
    (CountryRestrictHubModuleClient::new(env, &id), hub)
}

#[test]
fn eligibility_isolated_per_token_and_identity() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);

    // Each token has its own identity provider + allow-list.
    let id_a = env.register(IdentityMock, (admin.clone(),));
    let id_b = env.register(IdentityMock, (admin.clone(),));
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.configure(&ta, &id_a, &vec![&env, 840u32]); // token A allows US
    m.configure(&tb, &id_b, &vec![&env, 276u32]); // token B allows DE

    let alice = Address::generate(&env);
    IdentityMockClient::new(&env, &id_a).set_country(&alice, &840); // US on A's identity
    IdentityMockClient::new(&env, &id_b).set_country(&alice, &792); // TR on B's identity

    assert!(m.can_create(&alice, &1, &ta));  // US ∈ {US} on token A
    assert!(!m.can_create(&alice, &1, &tb)); // TR ∉ {DE} on token B — isolated
    // unattested recipient is denied
    let bob = Address::generate(&env);
    assert!(!m.can_create(&bob, &1, &ta));

    // can_transfer is both-party: sender AND recipient must be in the allow-list.
    IdentityMockClient::new(&env, &id_a).set_country(&bob, &840); // bob = US on A
    assert!(m.can_transfer(&alice, &bob, &1, &ta)); // both US ∈ {US} -> allowed
    let carol = Address::generate(&env); // unattested
    assert!(!m.can_transfer(&carol, &bob, &1, &ta)); // sender not attested -> denied
    assert!(!m.can_transfer(&alice, &carol, &1, &ta)); // recipient not attested -> denied
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn configure_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env); // no mock_all_auths
    m.configure(&Address::generate(&env), &Address::generate(&env), &vec![&env, 840u32]);
}
