#![no_std]
//! Mock identity/attribute provider — the attestor stand-in for the demo.
//!
//! Implements the `IdentityProvider` surface (`country_of`, `is_verified`). In the
//! demo, the admin (= us, the attestor) sets attributes; in production this is
//! replaced by a real KYC provider or the Phase-2 ZK-backed provider behind the
//! same interface.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
enum DataKey {
    Admin,
    Country(Address),
    Verified(Address),
}

#[contract]
pub struct IdentityMock;

#[contractimpl]
impl IdentityMock {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Attest an account's ISO-3166 numeric country code (also marks it verified).
    pub fn set_country(env: Env, account: Address, code: u32) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Country(account.clone()), &code);
        env.storage()
            .persistent()
            .set(&DataKey::Verified(account), &true);
    }

    pub fn set_verified(env: Env, account: Address, verified: bool) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Verified(account), &verified);
    }

    pub fn country_of(env: Env, account: Address) -> Option<u32> {
        env.storage().persistent().get(&DataKey::Country(account))
    }

    pub fn is_verified(env: Env, account: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Verified(account))
            .unwrap_or(false)
    }
}

impl IdentityMock {
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }
}
