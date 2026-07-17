#![no_std]
//! Multi-tenant MaxInvestorsPerCountry module: caps the number of distinct holders
//! attributed to any single country, per token. Combines a per-token balance mirror
//! (to detect holder transitions) with the token's per-token identity provider (to
//! bucket holders by country). All state keyed by (token, …); mutated only from the
//! hub's post-event fan-out (hub-authed).

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Hub,
    Identity(Address),     // token -> identity provider
    Cap(Address),          // token -> per-country cap
    Count(Address, u32),   // (token, country) -> distinct holders
    Bal(Address, Address), // (token, holder) -> balance
}

#[contract]
pub struct MaxInvestorsHubModule;

#[contractimpl]
impl MaxInvestorsHubModule {
    pub fn __constructor(env: Env, hub: Address) {
        env.storage().instance().set(&DataKey::Hub, &hub);
    }
    pub fn configure(env: Env, token: Address, identity: Address, cap: u32) {
        Self::require_hub(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Identity(token.clone()), &identity);
        env.storage().persistent().set(&DataKey::Cap(token), &cap);
    }
    pub fn set_cap(env: Env, token: Address, cap: u32) {
        Self::require_hub(&env);
        env.storage().persistent().set(&DataKey::Cap(token), &cap);
    }
    pub fn cap(env: Env, token: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Cap(token))
            .unwrap_or(0)
    }
    pub fn count(env: Env, token: Address, country: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Count(token, country))
            .unwrap_or(0)
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, amount: i128, token: Address) -> bool {
        if amount <= 0 {
            return true;
        }
        let country = match Self::country_of(&env, &token, &to) {
            Some(c) => c,
            None => return false,
        };
        if Self::bal(&env, &token, &to) > 0 {
            return true;
        }
        let from_frees_slot = Self::bal(&env, &token, &from) > 0
            && Self::bal(&env, &token, &from) - amount == 0
            && Self::country_of(&env, &token, &from) == Some(country);
        if from_frees_slot {
            return true;
        }
        Self::count(env.clone(), token.clone(), country) < Self::cap(env, token)
    }

    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        if amount <= 0 {
            return true;
        }
        let country = match Self::country_of(&env, &token, &to) {
            Some(c) => c,
            None => return false,
        };
        if Self::bal(&env, &token, &to) > 0 {
            return true;
        }
        Self::count(env.clone(), token.clone(), country) < Self::cap(env, token)
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

impl MaxInvestorsHubModule {
    fn require_hub(env: &Env) {
        let hub: Address = env.storage().instance().get(&DataKey::Hub).unwrap();
        hub.require_auth();
    }
    fn bal(env: &Env, token: &Address, who: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Bal(token.clone(), who.clone()))
            .unwrap_or(0)
    }
    fn country_of(env: &Env, token: &Address, who: &Address) -> Option<u32> {
        let identity: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Identity(token.clone()))
            .unwrap();
        IdentityClient::new(env, &identity).country_of(who)
    }
    fn bump_count(env: &Env, token: &Address, country: u32, delta: i32) {
        let key = DataKey::Count(token.clone(), country);
        let cur: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        let next = if delta < 0 {
            cur.saturating_sub((-delta) as u32)
        } else {
            cur + delta as u32
        };
        env.storage().persistent().set(&key, &next);
    }
    fn apply(env: &Env, token: &Address, who: &Address, delta: i128) {
        if delta == 0 {
            return;
        }
        let key = DataKey::Bal(token.clone(), who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new = old + delta;
        env.storage().persistent().set(&key, &new);
        if old <= 0 && new > 0 {
            if let Some(c) = Self::country_of(env, token, who) {
                Self::bump_count(env, token, c, 1);
            }
        } else if old > 0 && new <= 0 {
            if let Some(c) = Self::country_of(env, token, who) {
                Self::bump_count(env, token, c, -1);
            }
        }
    }
}
