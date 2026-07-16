#![no_std]
//! Lockup compliance module (self-contained, stateful, time-based).
//!
//! Each holder's acquired tokens are locked for `lock_seconds` from the moment they
//! receive them. A holder cannot transfer out until the lock elapses. Trustless:
//! uses only ledger time + on-chain bookkeeping.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Admin,
    Dispatcher,
    Duration,
    Acquired(Address),
}

#[contract]
pub struct LockupModule;

#[contractimpl]
impl LockupModule {
    pub fn __constructor(env: Env, admin: Address, dispatcher: Address, lock_seconds: u64) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Dispatcher, &dispatcher);
        env.storage().instance().set(&DataKey::Duration, &lock_seconds);
    }

    pub fn duration(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::Duration).unwrap()
    }

    /// Timestamp at which `who`'s tokens unlock (0 if never acquired).
    pub fn unlock_at(env: Env, who: Address) -> u64 {
        match env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::Acquired(who))
        {
            Some(acq) => {
                let dur: u64 = env.storage().instance().get(&DataKey::Duration).unwrap();
                acq + dur
            }
            None => 0,
        }
    }

    pub fn can_transfer(env: Env, from: Address, _to: Address, _amount: i128, _token: Address) -> bool {
        // Presence (not value 0) distinguishes "acquired" from "never held" — an
        // acquisition at ledger time 0 is still a real lock.
        match env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::Acquired(from))
        {
            Some(acq) => {
                let dur: u64 = env.storage().instance().get(&DataKey::Duration).unwrap();
                env.ledger().timestamp() >= acq + dur
            }
            None => true,
        }
    }

    pub fn can_create(_env: Env, _to: Address, _amount: i128, _token: Address) -> bool {
        true
    }

    pub fn transferred(env: Env, _from: Address, to: Address, _amount: i128, _token: Address) {
        Self::require_dispatcher(&env);
        Self::record(&env, &to);
    }

    pub fn created(env: Env, to: Address, _amount: i128, _token: Address) {
        Self::require_dispatcher(&env);
        Self::record(&env, &to);
    }

    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl LockupModule {
    fn require_dispatcher(env: &Env) {
        let d: Address = env.storage().instance().get(&DataKey::Dispatcher).unwrap();
        d.require_auth();
    }

    fn record(env: &Env, who: &Address) {
        let now = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Acquired(who.clone()), &now);
    }
}
