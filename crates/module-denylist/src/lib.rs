#![no_std]
//! Denylist (sanctions) compliance module (self-contained, stateful).
//!
//! Admin-managed blocklist of sanctioned addresses. Denies any transfer or mint
//! whose `from` or `to` is on the list. Stateless w.r.t. the identity layer — it
//! holds its own set and never reads the token or an identity provider, so there is
//! no re-entrancy concern.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Admin,
    Denied(Address),
}

#[contract]
pub struct DenylistModule;

#[contractimpl]
impl DenylistModule {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Add an address to the denylist. Admin-only.
    pub fn add_to_denylist(env: Env, account: Address) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Denied(account), &true);
    }

    /// Remove an address from the denylist. Admin-only.
    pub fn remove_from_denylist(env: Env, account: Address) {
        Self::require_admin(&env);
        env.storage().persistent().remove(&DataKey::Denied(account));
    }

    /// Whether an address is currently denied.
    pub fn is_denied(env: Env, account: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Denied(account))
            .unwrap_or(false)
    }

    pub fn can_transfer(
        env: Env,
        from: Address,
        to: Address,
        _amount: i128,
        _token: Address,
    ) -> bool {
        !Self::is_denied(env.clone(), from) && !Self::is_denied(env, to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, _token: Address) -> bool {
        !Self::is_denied(env, to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl DenylistModule {
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }
}
