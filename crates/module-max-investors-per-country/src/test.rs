#![cfg(test)]
//! Unit tests for the MaxInvestorsPerCountry compliance module.

use crate::{MaxInvestorsPerCountryModule, MaxInvestorsPerCountryModuleClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

const US: u32 = 840;
const DE: u32 = 276;

struct Fixture<'a> {
    env: Env,
    module: MaxInvestorsPerCountryModuleClient<'a>,
    identity: IdentityMockClient<'a>,
    token: Address,
}

fn setup(cap: u32) -> Fixture<'static> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let id = env.register(IdentityMock, (admin.clone(),));
    let identity = IdentityMockClient::new(&env, &id);
    let m = env.register(
        MaxInvestorsPerCountryModule,
        (admin.clone(), dispatcher, id.clone(), cap),
    );
    let module = MaxInvestorsPerCountryModuleClient::new(&env, &m);
    let token = Address::generate(&env);
    let _ = admin;
    Fixture {
        env,
        module,
        identity,
        token,
    }
}

/// Helper: a fresh address attested to `country`.
fn holder(f: &Fixture, country: u32) -> Address {
    let a = Address::generate(&f.env);
    f.identity.set_country(&a, &country);
    a
}

#[test]
fn cap_accessor_returns_configured() {
    let f = setup(3);
    assert_eq!(f.module.cap(), 3);
}

#[test]
fn set_cap_updates_cap() {
    let f = setup(3);
    f.module.set_cap(&5);
    assert_eq!(f.module.cap(), 5);
}

#[test]
#[should_panic]
fn set_cap_requires_admin() {
    // Register the module without blanket auth mocking so the admin
    // authorization on `set_cap` is actually required.
    let env = Env::default();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let id = env.register(IdentityMock, (admin.clone(),));
    let m = env.register(MaxInvestorsPerCountryModule, (admin, dispatcher, id, 3u32));
    let module = MaxInvestorsPerCountryModuleClient::new(&env, &m);
    module.set_cap(&5); // no admin auth provided → must panic
}

// A spoofer (arbitrary account, NOT the dispatcher) must not be able to drive a
// post-event directly. We do NOT mock the dispatcher's auth, so require_auth() on
// `dispatcher` must reject.
#[test]
#[should_panic]
fn created_rejects_non_dispatcher_caller() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let dispatcher = Address::generate(&env);
    let id = env.register(IdentityMock, (admin.clone(),));
    let m = env.register(MaxInvestorsPerCountryModule, (admin, dispatcher, id, 10u32));
    let module = MaxInvestorsPerCountryModuleClient::new(&env, &m);
    let who = Address::generate(&env);
    // No auth for `dispatcher` -> require_dispatcher() must panic.
    module.created(&who, &100, &Address::generate(&env));
}

#[test]
fn created_events_count_holders_per_country() {
    let f = setup(10);
    let a = holder(&f, US);
    let b = holder(&f, US);
    let c = holder(&f, DE);

    f.module.created(&a, &100, &f.token);
    f.module.created(&b, &100, &f.token);
    f.module.created(&c, &100, &f.token);

    assert_eq!(f.module.count(&US), 2);
    assert_eq!(f.module.count(&DE), 1);
}

#[test]
fn created_twice_for_same_holder_counts_once() {
    let f = setup(10);
    let a = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    f.module.created(&a, &50, &f.token);
    assert_eq!(f.module.count(&US), 1);
}

#[test]
fn can_create_allows_under_cap() {
    let f = setup(2);
    let a = holder(&f, US);
    assert!(f.module.can_create(&a, &100, &f.token));
}

#[test]
fn can_create_denies_new_holder_when_country_full() {
    let f = setup(2);
    let a = holder(&f, US);
    let b = holder(&f, US);
    let c = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    f.module.created(&b, &100, &f.token);
    assert!(
        !f.module.can_create(&c, &100, &f.token),
        "US at cap → new holder denied"
    );
}

#[test]
fn can_create_allows_existing_holder_when_country_full() {
    let f = setup(1);
    let a = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    // a is already a holder; topping up does not consume a new slot.
    assert!(f.module.can_create(&a, &50, &f.token));
}

#[test]
fn can_create_denies_unverified_recipient() {
    let f = setup(10);
    let stranger = Address::generate(&f.env); // no country attested
    assert!(
        !f.module.can_create(&stranger, &100, &f.token),
        "unattributable holder denied"
    );
}

#[test]
fn can_transfer_denies_new_holder_when_country_full() {
    let f = setup(1);
    let a = holder(&f, US);
    let b = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    // US already has 1 holder (a); b joining would exceed the cap.
    assert!(!f.module.can_transfer(&a, &b, &50, &f.token));
}

#[test]
fn can_transfer_allows_when_exiting_holder_frees_slot() {
    let f = setup(1);
    let a = holder(&f, US);
    let b = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    // Full transfer: a exits US (frees the slot) as b joins → net zero, allowed.
    assert!(f.module.can_transfer(&a, &b, &100, &f.token));
}

#[test]
fn destroyed_event_frees_a_holder_slot() {
    let f = setup(10);
    let a = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    assert_eq!(f.module.count(&US), 1);
    f.module.destroyed(&a, &100, &f.token);
    assert_eq!(f.module.count(&US), 0);
}

#[test]
fn transferred_event_moves_holder_between_addresses() {
    let f = setup(10);
    let a = holder(&f, US);
    let b = holder(&f, US);
    f.module.created(&a, &100, &f.token);
    assert_eq!(f.module.count(&US), 1);
    // a sends everything to b: a leaves, b joins → count stays 1.
    f.module.transferred(&a, &b, &100, &f.token);
    assert_eq!(f.module.count(&US), 1);
}
