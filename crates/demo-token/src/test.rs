#![cfg(test)]
//! End-to-end integration test: demo token + compliance dispatcher + all 4 modules.
//! Exercises pass and revert paths for MaxHolders, Lockup, MaxBalance, CountryRestrict.

use crate::{DemoToken, DemoTokenClient};
use constella_compliance::{Compliance, ComplianceClient};
use constella_identity_mock::{IdentityMock, IdentityMockClient};
use constella_module_country_restrict::CountryRestrictModule;
use constella_module_interface::ComplianceHook;
use constella_module_denylist::{DenylistModule, DenylistModuleClient};
use constella_module_lockup::LockupModule;
use constella_module_max_balance::MaxBalanceModule;
use constella_module_max_holders::{MaxHoldersModule, MaxHoldersModuleClient};
use constella_module_max_investors_per_country::{
    MaxInvestorsPerCountryModule, MaxInvestorsPerCountryModuleClient,
};
use constella_module_transfer_window::{TransferWindowModule, TransferWindowModuleClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{vec, Address, Env};

const US: u32 = 840;
const DE: u32 = 276;
const TR: u32 = 792;

#[test]
fn full_compliance_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let alice = Address::generate(&env); // US, allowed
    let bob = Address::generate(&env); // DE, allowed
    let carol = Address::generate(&env); // TR, disallowed
    let dave = Address::generate(&env); // unverified
    let eve = Address::generate(&env); // US, allowed
    let frank = Address::generate(&env); // DE, allowed

    // --- deploy identity provider (attestor) + compliance engine ---
    let identity = env.register(IdentityMock, (admin.clone(),));
    let identity_c = IdentityMockClient::new(&env, &identity);
    let compliance = env.register(Compliance, (admin.clone(),));
    let compliance_c = ComplianceClient::new(&env, &compliance);

    // --- deploy modules ---
    let max_holders = env.register(MaxHoldersModule, (admin.clone(), 3u32)); // cap 3 holders
    let lockup = env.register(LockupModule, (admin.clone(), 100u64)); // 100s lock
    let max_balance = env.register(MaxBalanceModule, (admin.clone(), 1000i128)); // cap 1000/holder
    let country = env.register(
        CountryRestrictModule,
        (admin.clone(), identity.clone(), vec![&env, US, DE]),
    );
    let mh_c = MaxHoldersModuleClient::new(&env, &max_holders);

    // --- deploy demo token wired to compliance ---
    let token_id = env.register(DemoToken, (admin.clone(), compliance.clone()));
    let token = DemoTokenClient::new(&env, &token_id);

    // --- register modules on hooks ---
    // MaxHolders needs every hook (checks + membership sync).
    for h in [
        ComplianceHook::CanCreate,
        ComplianceHook::CanTransfer,
        ComplianceHook::Created,
        ComplianceHook::Transferred,
        ComplianceHook::Destroyed,
    ] {
        compliance_c.add_module_to(&h, &max_holders);
    }
    for h in [
        ComplianceHook::CanTransfer,
        ComplianceHook::Created,
        ComplianceHook::Transferred,
    ] {
        compliance_c.add_module_to(&h, &lockup);
    }
    // MaxBalance maintains a balance mirror, so it also needs the post-event hooks.
    for h in [
        ComplianceHook::CanCreate,
        ComplianceHook::CanTransfer,
        ComplianceHook::Created,
        ComplianceHook::Transferred,
        ComplianceHook::Destroyed,
    ] {
        compliance_c.add_module_to(&h, &max_balance);
    }
    compliance_c.add_module_to(&ComplianceHook::CanTransfer, &country);
    compliance_c.add_module_to(&ComplianceHook::CanCreate, &country);

    // --- attestor sets identities ---
    identity_c.set_country(&alice, &US);
    identity_c.set_country(&bob, &DE);
    identity_c.set_country(&carol, &TR);
    identity_c.set_country(&eve, &US);
    identity_c.set_country(&frank, &DE);
    // dave intentionally left unverified

    // === CountryRestrict ===
    token.mint(&alice, &500); // US allowed
    token.mint(&bob, &500); // DE allowed
    assert_eq!(token.balance(&alice), 500);
    assert_eq!(token.balance(&bob), 500);
    assert_eq!(mh_c.holders(), 2);
    assert!(token.try_mint(&carol, &100).is_err()); // TR disallowed
    assert!(token.try_mint(&dave, &100).is_err()); // unverified

    // === MaxBalance (cap 1000) ===
    assert!(token.try_mint(&alice, &700).is_err()); // 500 + 700 > 1000

    // === Lockup (alice acquired at t=0, locked 100s) ===
    assert!(token.try_transfer(&alice, &bob, &100).is_err()); // still locked at t=0
    env.ledger().set_timestamp(200);
    token.transfer(&alice, &bob, &100); // lock elapsed
    assert_eq!(token.balance(&alice), 400);
    assert_eq!(token.balance(&bob), 600);

    // === MaxHolders (cap 3) ===
    token.mint(&eve, &100); // 3rd holder
    assert_eq!(mh_c.holders(), 3);
    assert!(token.try_mint(&frank, &100).is_err()); // would be 4th holder

    // free a slot: eve sends everything out -> drops below threshold
    env.ledger().set_timestamp(400); // past eve's lock (acquired t=200)
    token.transfer(&eve, &alice, &100);
    assert_eq!(token.balance(&eve), 0);
    assert_eq!(mh_c.holders(), 2); // eve removed
    token.mint(&frank, &100); // now allowed
    assert_eq!(mh_c.holders(), 3);

    // --- final state ---
    assert_eq!(token.balance(&alice), 500);
    assert_eq!(token.balance(&bob), 600);
    assert_eq!(token.balance(&eve), 0);
    assert_eq!(token.balance(&frank), 100);
    assert_eq!(token.total_supply(), 1200);
}

/// End-to-end integration for the Week 2/3 modules — Denylist,
/// MaxInvestorsPerCountry, and TransferWindow — composed through the dispatcher
/// on a single token. Exercises pass and revert paths for each, and confirms the
/// AND-combined dispatcher denies as soon as any one module objects.
#[test]
fn new_modules_compliance_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let alice = Address::generate(&env); // US
    let bob = Address::generate(&env); // US
    let carol = Address::generate(&env); // US
    let dave = Address::generate(&env); // DE

    // --- deploy identity provider + compliance engine ---
    let identity = env.register(IdentityMock, (admin.clone(),));
    let identity_c = IdentityMockClient::new(&env, &identity);
    let compliance = env.register(Compliance, (admin.clone(),));
    let compliance_c = ComplianceClient::new(&env, &compliance);

    // --- deploy the three new modules ---
    let denylist = env.register(DenylistModule, (admin.clone(),));
    let denylist_c = DenylistModuleClient::new(&env, &denylist);
    let investors = env.register(
        MaxInvestorsPerCountryModule,
        (admin.clone(), identity.clone(), 2u32), // cap 2 holders per country
    );
    let investors_c = MaxInvestorsPerCountryModuleClient::new(&env, &investors);
    let window = env.register(TransferWindowModule, (admin.clone(),));
    let window_c = TransferWindowModuleClient::new(&env, &window);

    // --- deploy demo token wired to compliance ---
    let token_id = env.register(DemoToken, (admin.clone(), compliance.clone()));
    let token = DemoTokenClient::new(&env, &token_id);

    // --- register modules on hooks ---
    // Denylist + TransferWindow are pure pre-checks.
    for m in [&denylist, &window] {
        compliance_c.add_module_to(&ComplianceHook::CanCreate, m);
        compliance_c.add_module_to(&ComplianceHook::CanTransfer, m);
    }
    // MaxInvestorsPerCountry maintains a mirror, so it needs all five hooks.
    for h in [
        ComplianceHook::CanCreate,
        ComplianceHook::CanTransfer,
        ComplianceHook::Created,
        ComplianceHook::Transferred,
        ComplianceHook::Destroyed,
    ] {
        compliance_c.add_module_to(&h, &investors);
    }

    // --- attestor sets identities ---
    identity_c.set_country(&alice, &US);
    identity_c.set_country(&bob, &US);
    identity_c.set_country(&carol, &US);
    identity_c.set_country(&dave, &DE);

    // === TransferWindow: freeze halts an otherwise-valid mint ===
    window_c.pause();
    assert!(window_c.is_paused());
    assert!(token.try_mint(&alice, &100).is_err()); // frozen
    window_c.unpause();
    token.mint(&alice, &100); // US holder 1

    // === Denylist: blocks mint of a denied recipient ===
    denylist_c.add_to_denylist(&dave);
    assert!(denylist_c.is_denied(&dave));
    assert!(token.try_mint(&dave, &100).is_err()); // dave denied
    denylist_c.remove_from_denylist(&dave);
    token.mint(&dave, &100); // DE holder 1

    token.mint(&bob, &100); // US holder 2 -> US now at cap
    assert_eq!(investors_c.count(&US), 2);
    assert_eq!(investors_c.count(&DE), 1);

    // === MaxInvestorsPerCountry: 3rd US holder denied ===
    assert!(token.try_mint(&carol, &100).is_err()); // US full

    // === Denylist: blocks a transfer to a denied recipient ===
    denylist_c.add_to_denylist(&bob);
    assert!(token.try_transfer(&alice, &bob, &50).is_err()); // bob denied
    denylist_c.remove_from_denylist(&bob);

    // === MaxInvestorsPerCountry: net-zero transfer lets carol replace alice ===
    token.transfer(&alice, &carol, &100); // alice exits US, carol joins US
    assert_eq!(token.balance(&alice), 0);
    assert_eq!(token.balance(&carol), 100);
    assert_eq!(investors_c.count(&US), 2); // still 2 (swap, not growth)

    // === TransferWindow: a future-only window closes transfers now ===
    window_c.set_window(&Some(1000), &None);
    assert_eq!(window_c.window(), (Some(1000), None));
    env.ledger().set_timestamp(500);
    assert!(token.try_transfer(&bob, &carol, &10).is_err()); // before open_from
    env.ledger().set_timestamp(1500);
    token.transfer(&bob, &carol, &10); // window now open
    assert_eq!(token.balance(&carol), 110);
    assert_eq!(token.balance(&bob), 90);

    // --- final state ---
    assert_eq!(token.total_supply(), 300);
    assert_eq!(investors_c.count(&US), 2);
    assert_eq!(investors_c.count(&DE), 1);
}
