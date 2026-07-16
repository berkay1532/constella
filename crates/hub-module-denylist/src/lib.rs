#![no_std]
//! Multi-tenant denylist module. One shared instance serves every token; all state is
//! keyed by (token, account). Mutations are accepted only from the hub (the hub gates
//! the issuer's authority per token); the module trusts the hub, stored at construction.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Denied(Address, Address), // (token, account)
}

#[contract]
pub struct DenylistHubModule;

#[contractimpl]
impl DenylistHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }

    pub fn add_to_denylist(env: Env, token: Address, account: Address) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Denied(token, account), &true);
    }

    pub fn remove_from_denylist(env: Env, token: Address, account: Address) {
        Self::require_hub(&env);
        env.storage().persistent().remove(&DataKey::Denied(token, account));
    }

    pub fn is_denied(env: Env, token: Address, account: Address) -> bool {
        env.storage().persistent().get(&DataKey::Denied(token, account)).unwrap_or(false)
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, _amount: i128, token: Address) -> bool {
        !Self::is_denied(env.clone(), token.clone(), from) && !Self::is_denied(env, token, to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        !Self::is_denied(env, token, to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl DenylistHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
}
