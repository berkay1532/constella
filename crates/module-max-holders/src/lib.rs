#![no_std]
//! MaxHolders compliance module (self-contained, stateful).
//!
//! Enforces a cap on the number of distinct holders. Maintains its own balance
//! mirror + holder count from the post-event hooks (Created/Transferred/Destroyed),
//! so it never re-enters the token contract (Soroban forbids re-entrancy). Trustless.
//!
//! Requirement: register this module on all five hooks before the first mint, so its
//! mirror stays consistent with the token from genesis.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
enum DataKey {
    Admin,
    Max,
    Count,
    Bal(Address),
}

#[contract]
pub struct MaxHoldersModule;

#[contractimpl]
impl MaxHoldersModule {
    pub fn __constructor(env: Env, admin: Address, max: u32) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Max, &max);
        env.storage().instance().set(&DataKey::Count, &0u32);
    }

    pub fn max(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Max).unwrap()
    }

    pub fn holders(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Count).unwrap_or(0)
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, _token: Address) -> bool {
        Self::allows(&env, &to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, _token: Address) -> bool {
        Self::allows(&env, &to)
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

impl MaxHoldersModule {
    fn bal(env: &Env, who: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Bal(who.clone()))
            .unwrap_or(0)
    }

    /// Allow if the recipient is already a holder, or there's room for a new one.
    fn allows(env: &Env, to: &Address) -> bool {
        if Self::bal(env, to) > 0 {
            return true;
        }
        let count: u32 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        let max: u32 = env.storage().instance().get(&DataKey::Max).unwrap();
        count < max
    }

    /// Apply a balance delta to the mirror and adjust the holder count on 0-crossings.
    fn apply(env: &Env, who: &Address, delta: i128) {
        if delta == 0 {
            return;
        }
        let key = DataKey::Bal(who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new = old + delta;
        env.storage().persistent().set(&key, &new);
        let mut count: u32 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        if old == 0 && new > 0 {
            count += 1;
            env.storage().instance().set(&DataKey::Count, &count);
        } else if old > 0 && new == 0 {
            count -= 1;
            env.storage().instance().set(&DataKey::Count, &count);
        }
    }
}
