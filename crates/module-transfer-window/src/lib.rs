#![no_std]
//! TransferWindow compliance module (admin-controlled freeze + time window).
//!
//! Two orthogonal admin controls over when the token may move:
//!
//! 1. **Pause / freeze** — the admin can halt all transfers *and* mints instantly
//!    (an emergency freeze), then resume.
//! 2. **Time window** — an optional `[open_from, open_until]` ledger-timestamp range
//!    outside of which transfers and mints are denied (e.g. a lockup that ends at a
//!    date, or a trading window that closes). Either bound may be left open.
//!
//! Stateless w.r.t. balances and identity — it reads only its own config and the
//! ledger clock, so there is no re-entrancy concern and no post-event bookkeeping.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
enum DataKey {
    Admin,
    Paused,
    Window,
}

#[contract]
pub struct TransferWindowModule;

#[contractimpl]
impl TransferWindowModule {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Halt all transfers and mints. Admin-only.
    pub fn pause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    /// Resume transfers and mints. Admin-only.
    pub fn unpause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Set the allowed time window (ledger timestamps). Either bound may be `None`
    /// for an open-ended side. Admin-only.
    pub fn set_window(env: Env, open_from: Option<u64>, open_until: Option<u64>) {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::Window, &(open_from, open_until));
    }

    /// Whether transfers/mints are currently frozen.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// The configured window as `(open_from, open_until)`.
    pub fn window(env: Env) -> (Option<u64>, Option<u64>) {
        env.storage()
            .instance()
            .get(&DataKey::Window)
            .unwrap_or((None, None))
    }

    pub fn can_transfer(
        env: Env,
        _from: Address,
        _to: Address,
        _amount: i128,
        _token: Address,
    ) -> bool {
        Self::is_open(&env)
    }

    pub fn can_create(env: Env, _to: Address, _amount: i128, _token: Address) -> bool {
        Self::is_open(&env)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl TransferWindowModule {
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    /// Movement is allowed iff not paused and the current ledger time is within the
    /// configured `[open_from, open_until]` window (bounds inclusive, either open-ended).
    fn is_open(env: &Env) -> bool {
        if Self::is_paused(env.clone()) {
            return false;
        }
        let (open_from, open_until) = Self::window(env.clone());
        let now = env.ledger().timestamp();
        if let Some(from) = open_from {
            if now < from {
                return false;
            }
        }
        if let Some(until) = open_until {
            if now > until {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod test;
