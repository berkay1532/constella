#![no_std]
//! Multi-tenant MaxBalance module: caps the balance any single holder may reach, per
//! token. One shared instance serves every token; the balance mirror and the per-token
//! cap are keyed by (token, …). The mirror is updated only from the hub's post-event
//! fan-out (hub-authed); pre-checks enforce bal(token,to) + amount <= max(token).

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Max(Address),          // token -> cap
    Bal(Address, Address), // (token, holder) -> balance
}

#[contract]
pub struct MaxBalanceHubModule;

#[contractimpl]
impl MaxBalanceHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }

    pub fn set_max(env: Env, token: Address, cap: i128) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Max(token), &cap);
    }

    pub fn max(env: Env, token: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Max(token)).unwrap_or(0)
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, amount: i128, token: Address) -> bool {
        Self::within_cap(&env, &token, &to, amount)
    }

    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        Self::within_cap(&env, &token, &to, amount)
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

impl MaxBalanceHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn bal(env: &Env, token: &Address, who: &Address) -> i128 {
        env.storage().persistent().get(&DataKey::Bal(token.clone(), who.clone())).unwrap_or(0)
    }
    fn within_cap(env: &Env, token: &Address, to: &Address, amount: i128) -> bool {
        Self::bal(env, token, to) + amount <= Self::max(env.clone(), token.clone())
    }
    fn apply(env: &Env, token: &Address, who: &Address, delta: i128) {
        if delta == 0 { return; }
        let key = DataKey::Bal(token.clone(), who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(old + delta));
    }
}
