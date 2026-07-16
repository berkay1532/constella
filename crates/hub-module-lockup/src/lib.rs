#![no_std]
//! Multi-tenant Lockup module: locks a holder's tokens for `duration(token)` seconds
//! from acquisition, per token. Shared instance; `Duration(token)` + `Acquired(token,
//! holder)` keyed by token; acquisition times recorded only from the hub's post-event
//! fan-out (hub-authed). Uses ledger time only — no balance mirror.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Duration(Address),          // token -> lock seconds
    Acquired(Address, Address), // (token, holder) -> ledger time
}

#[contract]
pub struct LockupHubModule;

#[contractimpl]
impl LockupHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn set_duration(env: Env, token: Address, secs: u64) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Duration(token), &secs);
    }
    pub fn unlock_at(env: Env, token: Address, holder: Address) -> u64 {
        match env.storage().persistent().get::<DataKey, u64>(&DataKey::Acquired(token.clone(), holder)) {
            Some(acq) => acq + Self::duration(&env, &token),
            None => 0,
        }
    }
    pub fn can_transfer(env: Env, from: Address, _to: Address, _amount: i128, token: Address) -> bool {
        match env.storage().persistent().get::<DataKey, u64>(&DataKey::Acquired(token.clone(), from)) {
            Some(acq) => env.ledger().timestamp() >= acq + Self::duration(&env, &token),
            None => true,
        }
    }
    pub fn can_create(_env: Env, _to: Address, _amount: i128, _token: Address) -> bool { true }
    pub fn transferred(env: Env, _from: Address, to: Address, _amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::record(&env, &token, &to);
    }
    pub fn created(env: Env, to: Address, _amount: i128, token: Address) {
        Self::require_hub(&env);
        Self::record(&env, &token, &to);
    }
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl LockupHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn duration(env: &Env, token: &Address) -> u64 {
        env.storage().persistent().get(&DataKey::Duration(token.clone())).unwrap_or(0)
    }
    fn record(env: &Env, token: &Address, who: &Address) {
        let now = env.ledger().timestamp();
        env.storage().persistent().set(&DataKey::Acquired(token.clone(), who.clone()), &now);
    }
}
