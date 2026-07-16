#![cfg(test)]
use crate::{MaxInvestorsHubModule, MaxInvestorsHubModuleClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const US: u32 = 840;
const DE: u32 = 276;

fn setup(env: &Env) -> (MaxInvestorsHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(MaxInvestorsHubModule, (hub.clone(),));
    (MaxInvestorsHubModuleClient::new(env, &id), hub)
}

#[test]
fn count_and_cap_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);
    let id_a = env.register(IdentityMock, (admin.clone(),));
    let id_b = env.register(IdentityMock, (admin.clone(),));
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    m.configure(&ta, &id_a, &2); // token A cap 2/country
    m.configure(&tb, &id_b, &1); // token B cap 1/country
    let ida = IdentityMockClient::new(&env, &id_a);
    let idb = IdentityMockClient::new(&env, &id_b);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    ida.set_country(&a, &US);
    ida.set_country(&b, &US);
    idb.set_country(&a, &US);

    m.created(&a, &100, &ta);
    m.created(&b, &100, &ta);
    assert_eq!(m.count(&ta, &US), 2);
    let c = Address::generate(&env);
    ida.set_country(&c, &US);
    assert!(!m.can_create(&c, &1, &ta)); // US full at 2 on token A
    // token B independent: its US count is 0
    assert!(m.can_create(&a, &1, &tb));
    // unattested recipient denied
    assert!(!m.can_create(&Address::generate(&env), &1, &ta));
    let _ = (DE,);
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
fn configure_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.configure(&Address::generate(&env), &Address::generate(&env), &1);
}
