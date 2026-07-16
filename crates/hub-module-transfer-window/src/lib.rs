#![no_std]
//! Multi-tenant TransferWindow module: admin freeze + time window, per token. Shared
//! instance; `Paused(token)` + `Window(token)` keyed by token. Reads only its config and
//! the ledger clock — no post-event bookkeeping.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Paused(Address),
    Window(Address),
}

#[contract]
pub struct TransferWindowHubModule;

#[contractimpl]
impl TransferWindowHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn pause(env: Env, token: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Paused(token), &true);
    }
    pub fn unpause(env: Env, token: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Paused(token), &false);
    }
    pub fn set_window(env: Env, token: Address, open_from: Option<u64>, open_until: Option<u64>) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Window(token), &(open_from, open_until));
    }
    pub fn is_paused(env: Env, token: Address) -> bool {
        env.storage().persistent().get(&DataKey::Paused(token)).unwrap_or(false)
    }
    pub fn window(env: Env, token: Address) -> (Option<u64>, Option<u64>) {
        env.storage().persistent().get(&DataKey::Window(token)).unwrap_or((None, None))
    }
    pub fn can_transfer(env: Env, _from: Address, _to: Address, _amount: i128, token: Address) -> bool {
        Self::is_open(&env, &token)
    }
    pub fn can_create(env: Env, _to: Address, _amount: i128, token: Address) -> bool {
        Self::is_open(&env, &token)
    }
    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl TransferWindowHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn is_open(env: &Env, token: &Address) -> bool {
        if Self::is_paused(env.clone(), token.clone()) { return false; }
        let (open_from, open_until) = Self::window(env.clone(), token.clone());
        let now = env.ledger().timestamp();
        if let Some(from) = open_from { if now < from { return false; } }
        if let Some(until) = open_until { if now > until { return false; } }
        true
    }
}
