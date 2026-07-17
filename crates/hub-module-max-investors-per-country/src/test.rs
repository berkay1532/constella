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

// The mirror's whole purpose is to track holder transitions on the POST-events. A same-country
// full transfer must keep the per-country count flat (one holder exits as another joins), and a
// burn that empties a holder must decrement the count. Exercises `transferred` and `destroyed`
// directly (the hub e2e is mint-only and never drives these paths).
#[test]
fn transfer_and_burn_update_per_country_count() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);
    let id = env.register(IdentityMock, (admin.clone(),));
    let t = Address::generate(&env);
    m.configure(&t, &id, &5);
    let idc = IdentityMockClient::new(&env, &id);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    idc.set_country(&alice, &US);
    idc.set_country(&bob, &US);

    m.created(&alice, &100, &t);
    assert_eq!(m.count(&t, &US), 1);
    // net-zero same-country full transfer: alice fully exits (count -1), bob joins (count +1) -> flat at 1
    m.transferred(&alice, &bob, &100, &t);
    assert_eq!(m.count(&t, &US), 1);
    // burn bob's whole balance -> bob crosses to 0 -> US count drops to 0
    m.destroyed(&bob, &100, &t);
    assert_eq!(m.count(&t, &US), 0);
}

// The net-zero exemption in `can_transfer`: at a full per-country cap, a full same-country
// transfer to a new holder is allowed (the slot the sender frees offsets the one the recipient
// takes), while a PARTIAL transfer — where the sender keeps a balance and the recipient is a
// fresh holder — is denied because it would consume a slot beyond the cap.
#[test]
fn can_transfer_net_zero_exemption_at_cap() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let admin = Address::generate(&env);
    let id = env.register(IdentityMock, (admin.clone(),));
    let t = Address::generate(&env);
    m.configure(&t, &id, &1); // cap 1
    let idc = IdentityMockClient::new(&env, &id);
    let alice = Address::generate(&env);
    let carol = Address::generate(&env);
    idc.set_country(&alice, &US);
    idc.set_country(&carol, &US);
    m.created(&alice, &100, &t); // US count 1 (at cap)
    // full transfer alice->carol (same country): net-zero -> allowed even at cap
    assert!(m.can_transfer(&alice, &carol, &100, &t));
    // partial transfer: alice keeps a balance, carol would be a NEW holder -> exceeds cap 1 -> denied
    assert!(!m.can_transfer(&alice, &carol, &40, &t));
}
