#![no_std]
//! Multi-tenant compliance hub. One instance serves every token: per-token issuer-admin,
//! a per-(token,hook) registry of shared module addresses, and a one-signature `launch`
//! that deploys the token (pointed at this hub) and wires the selected shared modules.

use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, BytesN, Env, Symbol, Vec};

#[cfg(test)]
mod test;

#[contracttype]
#[derive(Clone)]
pub struct LaunchConfig {
    pub admin: Address,
    pub denylist: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct LaunchResult {
    pub token: Address,
}

#[contracttype]
enum DataKey {
    PlatformAdmin,
    TokenWasm,
    ModuleAddr(Symbol),          // shared module address by kind
    Counter,
    TokenAdmin(Address),         // token -> issuer
    Modules(Address, Symbol),    // (token, hook) -> Vec<Address>
}

#[contract]
pub struct Hub;

#[contractimpl]
impl Hub {
    pub fn __constructor(env: Env, platform_admin: Address) {
        env.storage().instance().set(&DataKey::PlatformAdmin, &platform_admin);
    }

    pub fn set_token_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::TokenWasm, &hash);
    }

    pub fn set_module_addr(env: Env, kind: Symbol, addr: Address) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::ModuleAddr(kind), &addr);
    }

    pub fn token_admin(env: Env, token: Address) -> Address {
        env.storage().persistent().get(&DataKey::TokenAdmin(token)).unwrap()
    }

    pub fn modules_for(env: Env, token: Address, hook: Symbol) -> Vec<Address> {
        env.storage().persistent().get(&DataKey::Modules(token, hook)).unwrap_or(Vec::new(&env))
    }

    /// One-signature launch: deploy the token (admin = issuer, compliance = this hub) and
    /// wire the selected shared modules for that token.
    pub fn launch(env: Env, config: LaunchConfig) -> LaunchResult {
        config.admin.require_auth();
        let token_hash: BytesN<32> = env.storage().instance().get(&DataKey::TokenWasm).unwrap();
        let hub_addr = env.current_contract_address();
        let token = Self::deploy(&env, &token_hash, (config.admin.clone(), hub_addr));
        env.storage().persistent().set(&DataKey::TokenAdmin(token.clone()), &config.admin);

        if config.denylist {
            let m: Address = env.storage().instance().get(&DataKey::ModuleAddr(Symbol::new(&env, "denylist"))).unwrap();
            for h in ["CanCreate", "CanTransfer"] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
        }
        LaunchResult { token }
    }
}

impl Hub {
    fn require_platform_admin(env: &Env) {
        let a: Address = env.storage().instance().get(&DataKey::PlatformAdmin).unwrap();
        a.require_auth();
    }
    fn next_salt(env: &Env) -> BytesN<32> {
        let n: u32 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0);
        env.storage().instance().set(&DataKey::Counter, &(n + 1));
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&n.to_be_bytes());
        BytesN::from_array(env, &b)
    }
    fn deploy<A: soroban_sdk::ConstructorArgs>(env: &Env, hash: &BytesN<32>, args: A) -> Address {
        env.deployer().with_current_contract(Self::next_salt(env)).deploy_v2(hash.clone(), args)
    }
    fn register(env: &Env, token: &Address, hook: &Symbol, module: &Address) {
        let key = DataKey::Modules(token.clone(), hook.clone());
        let mut v: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![env]);
        v.push_back(module.clone());
        env.storage().persistent().set(&key, &v);
    }
}
