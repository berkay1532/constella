#![no_std]
//! Multi-tenant CountryRestrict module: only allows holders whose attested country
//! (from that token's own identity provider) is in the token's allow-list. A mint checks the
//! recipient; a transfer checks BOTH parties (sender and recipient must be in the allow-list).
//! One shared instance serves every token; identity + allow-list keyed by token. Reads only the
//! identity boundary (no balance mirror), so post-events are no-ops.

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Identity(Address), // token -> identity provider
    Allowed(Address),  // token -> allowed country codes
}

#[contract]
pub struct CountryRestrictHubModule;

#[contractimpl]
impl CountryRestrictHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }

    pub fn configure(env: Env, token: Address, identity: Address, allowed: Vec<u32>) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Identity(token.clone()), &identity);
        env.storage().persistent().set(&DataKey::Allowed(token), &allowed);
    }

    pub fn set_allowed(env: Env, token: Address, allowed: Vec<u32>) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Allowed(token), &allowed);
    }

    pub fn identity(env: Env, token: Address) -> Address {
        env.storage().persistent().get(&DataKey::Identity(token)).unwrap()
    }

    pub fn allowed(env: Env, token: Address) -> Vec<u32> {
        env.storage().persistent().get(&DataKey::Allowed(token)).unwrap()
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, _amount: i128, token: Address) -> bool {
        // Both parties' countries must be in the allow-list: the sender and the recipient.
        Self::eligible(&env, &token, &from) && Self::eligible(&env, &token, &to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl CountryRestrictHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn eligible(env: &Env, token: &Address, who: &Address) -> bool {
        let identity: Address = env.storage().persistent().get(&DataKey::Identity(token.clone())).unwrap();
        let allowed: Vec<u32> = env.storage().persistent().get(&DataKey::Allowed(token.clone())).unwrap();
        match IdentityClient::new(env, &identity).country_of(who) {
            Some(code) => allowed.iter().any(|c| c == code),
            None => false,
        }
    }
}
