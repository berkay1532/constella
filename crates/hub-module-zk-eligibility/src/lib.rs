#![no_std]
//! Multi-tenant ZK-eligibility module: gates transfers on the recipient's ZK eligibility
//! flag (`is_verified`) read from the token's per-token ZK identity — never a cleartext
//! country. A disallowed recipient shows up as "not eligible"; their country is never read
//! or revealed. Stateless gate; all config keyed by token.

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Identity(Address), // token -> its ZK identity provider
}

#[contract]
pub struct ZkEligibilityHubModule;

#[contractimpl]
impl ZkEligibilityHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn configure(env: Env, token: Address, identity: Address) {
        Self::require_hub(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Identity(token), &identity);
    }
    pub fn identity(env: Env, token: Address) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Identity(token))
            .unwrap()
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }
    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, token: Address) -> bool {
        Self::eligible(&env, &token, &to)
    }
    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl ZkEligibilityHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn eligible(env: &Env, token: &Address, who: &Address) -> bool {
        let id: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Identity(token.clone()))
            .unwrap();
        IdentityClient::new(env, &id).is_verified(who)
    }
}
