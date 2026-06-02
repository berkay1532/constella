#![no_std]
//! Constella compliance dispatcher (the "engine").
//!
//! Holds a list of module addresses per [`ComplianceHook`] and runs them on each
//! hook. Pre-checks (`can_transfer`/`can_create`) are AND-combined across modules;
//! post-events (`transferred`/`created`/`destroyed`) are fanned out. Mirrors the
//! OpenZeppelin RWA compliance surface so the same modules port to OZ later.

use constella_module_interface::{ComplianceError, ComplianceHook, ModuleClient};
use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, Address, Env, Vec};

#[contracttype]
enum DataKey {
    Admin,
    Modules(ComplianceHook),
}

#[contract]
pub struct Compliance;

#[contractimpl]
impl Compliance {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    /// Register `module` to run on `hook`. Admin-only.
    pub fn add_module_to(env: Env, hook: ComplianceHook, module: Address) {
        Self::require_admin(&env);
        let key = DataKey::Modules(hook);
        let mut v: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        if v.iter().any(|m| m == module) {
            panic_with_error!(&env, ComplianceError::ModuleAlreadyRegistered);
        }
        v.push_back(module);
        env.storage().persistent().set(&key, &v);
    }

    /// Unregister `module` from `hook`. Admin-only.
    pub fn remove_module_from(env: Env, hook: ComplianceHook, module: Address) {
        Self::require_admin(&env);
        let key = DataKey::Modules(hook);
        let v: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        let mut out = Vec::new(&env);
        let mut found = false;
        for m in v.iter() {
            if m == module {
                found = true;
            } else {
                out.push_back(m);
            }
        }
        if !found {
            panic_with_error!(&env, ComplianceError::ModuleNotRegistered);
        }
        env.storage().persistent().set(&key, &out);
    }

    pub fn get_modules_for_hook(env: Env, hook: ComplianceHook) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Modules(hook))
            .unwrap_or(Vec::new(&env))
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, amount: i128, token: Address) -> bool {
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::CanTransfer);
        for m in mods.iter() {
            if !ModuleClient::new(&env, &m).can_transfer(&from, &to, &amount, &token) {
                return false;
            }
        }
        true
    }

    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::CanCreate);
        for m in mods.iter() {
            if !ModuleClient::new(&env, &m).can_create(&to, &amount, &token) {
                return false;
            }
        }
        true
    }

    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address) {
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::Transferred);
        for m in mods.iter() {
            ModuleClient::new(&env, &m).transferred(&from, &to, &amount, &token);
        }
    }

    pub fn created(env: Env, to: Address, amount: i128, token: Address) {
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::Created);
        for m in mods.iter() {
            ModuleClient::new(&env, &m).created(&to, &amount, &token);
        }
    }

    pub fn destroyed(env: Env, from: Address, amount: i128, token: Address) {
        let mods = Self::get_modules_for_hook(env.clone(), ComplianceHook::Destroyed);
        for m in mods.iter() {
            ModuleClient::new(&env, &m).destroyed(&from, &amount, &token);
        }
    }
}

impl Compliance {
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }
}
