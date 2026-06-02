#![no_std]
//! MaxBalance compliance module (self-contained, stateful mirror).
//!
//! Caps the balance any single holder may reach (concentration limit). Maintains its
//! own balance mirror from the post-event hooks rather than re-entering the token
//! (Soroban forbids re-entrancy). Trustless.
//!
//! Requirement: register on all five hooks before the first mint so the mirror stays
//! consistent with the token from genesis.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
enum DataKey {
    Admin,
    Max,
    Bal(Address),
}

#[contract]
pub struct MaxBalanceModule;

#[contractimpl]
impl MaxBalanceModule {
    pub fn __constructor(env: Env, admin: Address, max_per_holder: i128) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Max, &max_per_holder);
    }

    pub fn max(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::Max).unwrap()
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, amount: i128, _token: Address) -> bool {
        Self::within_cap(&env, &to, amount)
    }

    pub fn can_create(env: Env, to: Address, amount: i128, _token: Address) -> bool {
        Self::within_cap(&env, &to, amount)
    }

    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, _token: Address) {
        Self::apply(&env, &from, -amount);
        Self::apply(&env, &to, amount);
    }

    pub fn created(env: Env, to: Address, amount: i128, _token: Address) {
        Self::apply(&env, &to, amount);
    }

    pub fn destroyed(env: Env, from: Address, amount: i128, _token: Address) {
        Self::apply(&env, &from, -amount);
    }
}

impl MaxBalanceModule {
    fn bal(env: &Env, who: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Bal(who.clone()))
            .unwrap_or(0)
    }

    fn within_cap(env: &Env, to: &Address, amount: i128) -> bool {
        let max: i128 = env.storage().instance().get(&DataKey::Max).unwrap();
        Self::bal(env, to) + amount <= max
    }

    fn apply(env: &Env, who: &Address, delta: i128) {
        if delta == 0 {
            return;
        }
        let key = DataKey::Bal(who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(old + delta));
    }
}
