#![no_std]
//! Multi-tenant MaxHolders module: caps the number of distinct holders per token.
//! One shared instance; balance mirror + holder count keyed by (token, …), updated only
//! from the hub's post-event fan-out (hub-authed).

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Max(Address),          // token -> cap
    Count(Address),        // token -> distinct holders
    Bal(Address, Address), // (token, holder) -> balance
}

#[contract]
pub struct MaxHoldersHubModule;

#[contractimpl]
impl MaxHoldersHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn set_max(env: Env, token: Address, cap: u32) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Max(token), &cap);
    }
    pub fn max(env: Env, token: Address) -> u32 {
        env.storage().persistent().get(&DataKey::Max(token)).unwrap_or(0)
    }
    pub fn holders(env: Env, token: Address) -> u32 {
        env.storage().persistent().get(&DataKey::Count(token)).unwrap_or(0)
    }
    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, token: Address) -> bool {
        Self::allows(&env, &token, &to)
    }
    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        Self::allows(&env, &token, &to)
    }
    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::apply(&env, &token, &from, -amount);
        Self::apply(&env, &token, &to, amount);
    }
    pub fn created(env: Env, to: Address, amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::apply(&env, &token, &to, amount);
    }
    pub fn destroyed(env: Env, from: Address, amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::apply(&env, &token, &from, -amount);
    }
}

impl MaxHoldersHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn bal(env: &Env, token: &Address, who: &Address) -> i128 {
        env.storage().persistent().get(&DataKey::Bal(token.clone(), who.clone())).unwrap_or(0)
    }
    fn allows(env: &Env, token: &Address, to: &Address) -> bool {
        if Self::bal(env, token, to) > 0 { return true; }
        let count: u32 = env.storage().persistent().get(&DataKey::Count(token.clone())).unwrap_or(0);
        let max: u32 = env.storage().persistent().get(&DataKey::Max(token.clone())).unwrap_or(0);
        count < max
    }
    fn apply(env: &Env, token: &Address, who: &Address, delta: i128) {
        if delta == 0 { return; }
        let key = DataKey::Bal(token.clone(), who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new = old + delta;
        env.storage().persistent().set(&key, &new);
        let ckey = DataKey::Count(token.clone());
        let mut count: u32 = env.storage().persistent().get(&ckey).unwrap_or(0);
        if old == 0 && new > 0 { count += 1; env.storage().persistent().set(&ckey, &count); }
        else if old > 0 && new == 0 { count -= 1; env.storage().persistent().set(&ckey, &count); }
    }
}
