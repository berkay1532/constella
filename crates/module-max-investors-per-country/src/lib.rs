#![no_std]
//! MaxInvestorsPerCountry compliance module (identity-dependent, stateful mirror).
//!
//! Caps the number of *distinct holders* attributed to any single country (a common
//! reg-A/reg-S style investor-count limit). Two ingredients:
//!
//! 1. **Identity** — reads `country_of(account)` from the configured `IdentityProvider`
//!    to bucket each holder into a country.
//! 2. **Balance mirror** — tracks each address's balance from the post-event hooks so
//!    it can detect holder transitions (0 → positive = joins, positive → 0 = leaves)
//!    without re-entering the token. Per-country holder counts are derived from those
//!    transitions.
//!
//! Requirement: register on all five hooks before the first mint so the mirror and the
//! per-country counts stay consistent with the token from genesis.

use constella_module_interface::IdentityClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
enum DataKey {
    Admin,
    Dispatcher,
    Identity,
    Cap,
    Bal(Address),
    Count(u32),
}

#[contract]
pub struct MaxInvestorsPerCountryModule;

#[contractimpl]
impl MaxInvestorsPerCountryModule {
    pub fn __constructor(
        env: Env,
        admin: Address,
        dispatcher: Address,
        identity: Address,
        cap: u32,
    ) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Dispatcher, &dispatcher);
        env.storage().instance().set(&DataKey::Identity, &identity);
        env.storage().instance().set(&DataKey::Cap, &cap);
    }

    /// Update the per-country holder cap. Admin-only.
    pub fn set_cap(env: Env, cap: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::Cap, &cap);
    }

    /// The per-country holder cap.
    pub fn cap(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Cap).unwrap()
    }

    /// Current number of distinct holders attributed to `country`.
    pub fn count(env: Env, country: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Count(country))
            .unwrap_or(0)
    }

    pub fn can_transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
        _token: Address,
    ) -> bool {
        if amount <= 0 {
            return true;
        }
        // `to` must be attributable to a country to be counted against the cap.
        let country = match Self::country_of(&env, &to) {
            Some(c) => c,
            None => return false,
        };
        // If `to` is already a holder, no new slot is consumed.
        if Self::bal(&env, &to) > 0 {
            return true;
        }
        // `to` would join `country`. If `from` fully exits that same country the slot
        // it frees offsets the one `to` takes (net zero) — always allowed.
        let from_frees_slot = Self::bal(&env, &from) > 0
            && Self::bal(&env, &from) - amount == 0
            && Self::country_of(&env, &from) == Some(country);
        if from_frees_slot {
            return true;
        }
        // Otherwise `to` consumes a fresh slot: room only if below the cap.
        Self::count(env.clone(), country) < Self::cap(env)
    }

    pub fn can_create(env: Env, to: Address, amount: i128, _token: Address) -> bool {
        if amount <= 0 {
            return true;
        }
        let country = match Self::country_of(&env, &to) {
            Some(c) => c,
            None => return false,
        };
        // Existing holders consume no new slot.
        if Self::bal(&env, &to) > 0 {
            return true;
        }
        Self::count(env.clone(), country) < Self::cap(env)
    }

    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, _token: Address) {
        Self::require_dispatcher(&env);
        Self::apply(&env, &from, -amount);
        Self::apply(&env, &to, amount);
    }

    pub fn created(env: Env, to: Address, amount: i128, _token: Address) {
        Self::require_dispatcher(&env);
        Self::apply(&env, &to, amount);
    }

    pub fn destroyed(env: Env, from: Address, amount: i128, _token: Address) {
        Self::require_dispatcher(&env);
        Self::apply(&env, &from, -amount);
    }
}

impl MaxInvestorsPerCountryModule {
    fn require_dispatcher(env: &Env) {
        let d: Address = env.storage().instance().get(&DataKey::Dispatcher).unwrap();
        d.require_auth();
    }

    fn bal(env: &Env, who: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Bal(who.clone()))
            .unwrap_or(0)
    }

    fn country_of(env: &Env, who: &Address) -> Option<u32> {
        let identity: Address = env.storage().instance().get(&DataKey::Identity).unwrap();
        IdentityClient::new(env, &identity).country_of(who)
    }

    fn bump_count(env: &Env, country: u32, delta: i32) {
        let key = DataKey::Count(country);
        let cur: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        let next = if delta < 0 {
            cur.saturating_sub((-delta) as u32)
        } else {
            cur + delta as u32
        };
        env.storage().persistent().set(&key, &next);
    }

    /// Apply a balance delta to the mirror and, on a holder transition
    /// (0 → positive = join, positive → 0 = leave), adjust the holder's
    /// per-country count.
    fn apply(env: &Env, who: &Address, delta: i128) {
        if delta == 0 {
            return;
        }
        let key = DataKey::Bal(who.clone());
        let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let new = old + delta;
        env.storage().persistent().set(&key, &new);

        if old <= 0 && new > 0 {
            if let Some(c) = Self::country_of(env, who) {
                Self::bump_count(env, c, 1);
            }
        } else if old > 0 && new <= 0 {
            if let Some(c) = Self::country_of(env, who) {
                Self::bump_count(env, c, -1);
            }
        }
    }
}

#[cfg(test)]
mod test;
