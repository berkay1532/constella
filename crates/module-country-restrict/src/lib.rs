#![no_std]
//! CountryRestrict compliance module (identity-dependent).
//!
//! Only allows holders whose attested country (from an `IdentityProvider`) is in the
//! configured allow-list. This is the one module that depends on the attestor
//! boundary — it reads `country_of(to)` from the identity provider and denies if the
//! recipient is unverified or in a disallowed country.

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

#[contracttype]
enum DataKey {
    Admin,
    Identity,
    Allowed,
}

#[contract]
pub struct CountryRestrictModule;

#[contractimpl]
impl CountryRestrictModule {
    pub fn __constructor(env: Env, admin: Address, identity: Address, allowed: Vec<u32>) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Identity, &identity);
        env.storage().instance().set(&DataKey::Allowed, &allowed);
    }

    pub fn identity(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Identity).unwrap()
    }

    pub fn allowed(env: Env) -> Vec<u32> {
        env.storage().instance().get(&DataKey::Allowed).unwrap()
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, _token: Address) -> bool {
        Self::eligible(&env, &to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, _token: Address) -> bool {
        Self::eligible(&env, &to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl CountryRestrictModule {
    fn eligible(env: &Env, who: &Address) -> bool {
        let identity: Address = env.storage().instance().get(&DataKey::Identity).unwrap();
        let allowed: Vec<u32> = env.storage().instance().get(&DataKey::Allowed).unwrap();
        match IdentityClient::new(env, &identity).country_of(who) {
            Some(code) => allowed.iter().any(|c| c == code),
            None => false,
        }
    }
}
