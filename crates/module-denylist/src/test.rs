#![cfg(test)]
//! Unit tests for the Denylist (sanctions) compliance module.

use crate::{DenylistModule, DenylistModuleClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

fn setup(env: &Env) -> (DenylistModuleClient<'_>, Address) {
    let admin = Address::generate(env);
    let id = env.register(DenylistModule, (admin.clone(),));
    (DenylistModuleClient::new(env, &id), admin)
}

#[test]
fn is_denied_tracks_add_and_remove() {
    let env = Env::default();
    env.mock_all_auths();
    let (module, _admin) = setup(&env);
    let account = Address::generate(&env);

    assert!(!module.is_denied(&account), "unknown address is not denied");

    module.add_to_denylist(&account);
    assert!(module.is_denied(&account), "added address is denied");

    module.remove_from_denylist(&account);
    assert!(!module.is_denied(&account), "removed address is not denied");
}

#[test]
fn can_transfer_denies_denied_recipient() {
    let env = Env::default();
    env.mock_all_auths();
    let (module, _admin) = setup(&env);
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env);

    module.add_to_denylist(&to);
    assert!(!module.can_transfer(&from, &to, &1, &token));
}

#[test]
fn can_transfer_denies_denied_sender() {
    let env = Env::default();
    env.mock_all_auths();
    let (module, _admin) = setup(&env);
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env);

    module.add_to_denylist(&from);
    assert!(!module.can_transfer(&from, &to, &1, &token));
}

#[test]
fn can_transfer_allows_clean_parties() {
    let env = Env::default();
    env.mock_all_auths();
    let (module, _admin) = setup(&env);
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env);

    assert!(module.can_transfer(&from, &to, &1, &token));
}

#[test]
fn can_create_denies_denied_recipient() {
    let env = Env::default();
    env.mock_all_auths();
    let (module, _admin) = setup(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env);

    assert!(
        module.can_create(&to, &1, &token),
        "clean recipient may receive a mint"
    );

    module.add_to_denylist(&to);
    assert!(
        !module.can_create(&to, &1, &token),
        "denied recipient may not receive a mint"
    );
}

#[test]
#[should_panic]
fn add_to_denylist_requires_admin() {
    let env = Env::default();
    // No mock_all_auths: the admin authorization is not provided, so the
    // admin-gated mutator must reject the call.
    let (module, _admin) = setup(&env);
    let account = Address::generate(&env);
    module.add_to_denylist(&account);
}
