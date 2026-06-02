#![no_std]
//! Minimal SEP-41-style permissioned fungible token for the Constella demo.
//!
//! On every `mint`/`transfer` it calls the compliance dispatcher's pre-check
//! (`can_create`/`can_transfer`) and reverts if denied, then runs the post-event
//! (`created`/`transferred`). This mirrors the OpenZeppelin RWA token transfer flow;
//! in production the OZ RWA token plays this role.

use constella_module_interface::{ComplianceError, ModuleClient};
use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
enum DataKey {
    Admin,
    Compliance,
    Supply,
    Balance(Address),
}

#[contract]
pub struct DemoToken;

#[contractimpl]
impl DemoToken {
    pub fn __constructor(env: Env, admin: Address, compliance: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Compliance, &compliance);
        env.storage().instance().set(&DataKey::Supply, &0i128);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(id))
            .unwrap_or(0)
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::Supply).unwrap_or(0)
    }

    pub fn compliance(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Compliance).unwrap()
    }

    /// Admin-only issuance. Runs the compliance `can_create`/`created` hooks.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        let token = env.current_contract_address();
        let c = ModuleClient::new(&env, &Self::compliance(env.clone()));
        if !c.can_create(&to, &amount, &token) {
            panic_with_error!(&env, ComplianceError::Denied);
        }
        let b = Self::balance(env.clone(), to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(b + amount));
        let s: i128 = env.storage().instance().get(&DataKey::Supply).unwrap_or(0);
        env.storage().instance().set(&DataKey::Supply, &(s + amount));
        c.created(&to, &amount, &token);
    }

    /// Holder-authorized transfer. Runs the compliance `can_transfer`/`transferred` hooks.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let token = env.current_contract_address();
        let c = ModuleClient::new(&env, &Self::compliance(env.clone()));
        if !c.can_transfer(&from, &to, &amount, &token) {
            panic_with_error!(&env, ComplianceError::Denied);
        }
        let bf = Self::balance(env.clone(), from.clone());
        if bf < amount {
            panic_with_error!(&env, ComplianceError::Denied);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(bf - amount));
        let bt = Self::balance(env.clone(), to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(bt + amount));
        c.transferred(&from, &to, &amount, &token);
    }
}
