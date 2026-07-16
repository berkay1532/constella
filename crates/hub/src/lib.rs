#![no_std]
//! Multi-tenant compliance hub. One instance serves every token: per-token issuer-admin,
//! a per-(token,hook) registry of shared module addresses, and a one-signature `launch`
//! that deploys the token (pointed at this hub) and wires the selected shared modules.

use constella_module_interface::{
    CountryRestrictClient, DenylistClient, MaxBalanceClient, ModuleClient,
};
use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, BytesN, Env, Symbol, Vec};

#[cfg(test)]
mod test;

/// Single source of truth for the hook-name strings used to key `DataKey::Modules(token, hook)`.
/// `launch` (register side) and the hook fan-out (read side) both go through these constants so
/// a typo can't silently desync registration from dispatch.
mod hooks {
    pub const CAN_CREATE: &str = "CanCreate";
    pub const CAN_TRANSFER: &str = "CanTransfer";
    pub const TRANSFERRED: &str = "Transferred";
    pub const CREATED: &str = "Created";
    pub const DESTROYED: &str = "Destroyed";
}

#[contracttype]
#[derive(Clone)]
pub struct LaunchConfig {
    pub admin: Address,
    pub denylist: bool,
    pub max_balance: i128,          // 0 = not selected
    pub country_restrict: Vec<u32>, // empty = not selected
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
    IdentityWasm,
    ModuleAddr(Symbol), // shared module address by kind
    Counter,
    TokenAdmin(Address),      // token -> issuer
    Modules(Address, Symbol), // (token, hook) -> Vec<Address>
    Identity(Address),        // token -> its own per-token identity instance
}

#[contract]
pub struct Hub;

#[contractimpl]
impl Hub {
    pub fn __constructor(env: Env, platform_admin: Address) {
        env.storage()
            .instance()
            .set(&DataKey::PlatformAdmin, &platform_admin);
    }

    pub fn set_token_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::TokenWasm, &hash);
    }

    pub fn set_identity_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::IdentityWasm, &hash);
    }

    pub fn set_module_addr(env: Env, kind: Symbol, addr: Address) {
        Self::require_platform_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::ModuleAddr(kind), &addr);
    }

    pub fn token_admin(env: Env, token: Address) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::TokenAdmin(token))
            .unwrap()
    }

    pub fn modules_for(env: Env, token: Address, hook: Symbol) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Modules(token, hook))
            .unwrap_or(Vec::new(&env))
    }

    /// One-signature launch: deploy the token (admin = issuer, compliance = this hub) and
    /// wire the selected shared modules for that token.
    pub fn launch(env: Env, config: LaunchConfig) -> LaunchResult {
        config.admin.require_auth();
        let token_hash: BytesN<32> = env.storage().instance().get(&DataKey::TokenWasm).unwrap();
        let hub_addr = env.current_contract_address();
        let token = Self::deploy(&env, &token_hash, (config.admin.clone(), hub_addr));
        env.storage()
            .persistent()
            .set(&DataKey::TokenAdmin(token.clone()), &config.admin);

        if config.denylist {
            let m = Self::module_addr(&env, "denylist");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
        }
        if config.max_balance > 0 {
            let m = Self::module_addr(&env, "max_balance");
            for h in [
                hooks::CAN_CREATE,
                hooks::CAN_TRANSFER,
                hooks::CREATED,
                hooks::TRANSFERRED,
                hooks::DESTROYED,
            ] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            MaxBalanceClient::new(&env, &m).set_max(&token, &config.max_balance);
        }
        if !config.country_restrict.is_empty() {
            let identity_hash: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::IdentityWasm)
                .unwrap();
            let identity = Self::deploy(&env, &identity_hash, (config.admin.clone(),));
            env.storage()
                .persistent()
                .set(&DataKey::Identity(token.clone()), &identity);
            let m = Self::module_addr(&env, "country_restrict");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            CountryRestrictClient::new(&env, &m).configure(
                &token,
                &identity,
                &config.country_restrict,
            );
        }
        LaunchResult { token }
    }

    // --- hook surface (called by the token; matches the ModuleClient ABI) ---
    pub fn can_transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
        token: Address,
    ) -> bool {
        for m in Self::modules_for(
            env.clone(),
            token.clone(),
            Symbol::new(&env, hooks::CAN_TRANSFER),
        )
        .iter()
        {
            if !ModuleClient::new(&env, &m).can_transfer(&from, &to, &amount, &token) {
                return false;
            }
        }
        true
    }
    pub fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool {
        for m in Self::modules_for(
            env.clone(),
            token.clone(),
            Symbol::new(&env, hooks::CAN_CREATE),
        )
        .iter()
        {
            if !ModuleClient::new(&env, &m).can_create(&to, &amount, &token) {
                return false;
            }
        }
        true
    }
    pub fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address) {
        token.require_auth();
        for m in Self::modules_for(
            env.clone(),
            token.clone(),
            Symbol::new(&env, hooks::TRANSFERRED),
        )
        .iter()
        {
            ModuleClient::new(&env, &m).transferred(&from, &to, &amount, &token);
        }
    }
    pub fn created(env: Env, to: Address, amount: i128, token: Address) {
        token.require_auth();
        for m in Self::modules_for(
            env.clone(),
            token.clone(),
            Symbol::new(&env, hooks::CREATED),
        )
        .iter()
        {
            ModuleClient::new(&env, &m).created(&to, &amount, &token);
        }
    }
    pub fn destroyed(env: Env, from: Address, amount: i128, token: Address) {
        token.require_auth();
        for m in Self::modules_for(
            env.clone(),
            token.clone(),
            Symbol::new(&env, hooks::DESTROYED),
        )
        .iter()
        {
            ModuleClient::new(&env, &m).destroyed(&from, &amount, &token);
        }
    }

    // --- issuer forwarders (single auth surface: Admin(token).require_auth) ---
    pub fn add_to_denylist(env: Env, token: Address, account: Address) {
        Self::require_token_admin(&env, &token);
        DenylistClient::new(&env, &Self::module_addr(&env, "denylist"))
            .add_to_denylist(&token, &account);
    }
    pub fn remove_from_denylist(env: Env, token: Address, account: Address) {
        Self::require_token_admin(&env, &token);
        DenylistClient::new(&env, &Self::module_addr(&env, "denylist"))
            .remove_from_denylist(&token, &account);
    }
    pub fn is_denied(env: Env, token: Address, account: Address) -> bool {
        DenylistClient::new(&env, &Self::module_addr(&env, "denylist")).is_denied(&token, &account)
    }
    pub fn set_max_balance(env: Env, token: Address, cap: i128) {
        Self::require_token_admin(&env, &token);
        MaxBalanceClient::new(&env, &Self::module_addr(&env, "max_balance")).set_max(&token, &cap);
    }
    pub fn max_balance(env: Env, token: Address) -> i128 {
        MaxBalanceClient::new(&env, &Self::module_addr(&env, "max_balance")).max(&token)
    }
    pub fn identity(env: Env, token: Address) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Identity(token))
            .unwrap()
    }
    pub fn set_country_allow(env: Env, token: Address, codes: Vec<u32>) {
        Self::require_token_admin(&env, &token);
        CountryRestrictClient::new(&env, &Self::module_addr(&env, "country_restrict"))
            .set_allowed(&token, &codes);
    }
}

impl Hub {
    fn require_platform_admin(env: &Env) {
        let a: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlatformAdmin)
            .unwrap();
        a.require_auth();
    }
    fn require_token_admin(env: &Env, token: &Address) {
        let a: Address = env
            .storage()
            .persistent()
            .get(&DataKey::TokenAdmin(token.clone()))
            .unwrap();
        a.require_auth();
    }
    fn module_addr(env: &Env, kind: &str) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::ModuleAddr(Symbol::new(env, kind)))
            .unwrap()
    }
    fn next_salt(env: &Env) -> BytesN<32> {
        let n: u32 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0);
        env.storage().instance().set(&DataKey::Counter, &(n + 1));
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&n.to_be_bytes());
        BytesN::from_array(env, &b)
    }
    fn deploy<A: soroban_sdk::ConstructorArgs>(env: &Env, hash: &BytesN<32>, args: A) -> Address {
        env.deployer()
            .with_current_contract(Self::next_salt(env))
            .deploy_v2(hash.clone(), args)
    }
    fn register(env: &Env, token: &Address, hook: &Symbol, module: &Address) {
        let key = DataKey::Modules(token.clone(), hook.clone());
        let mut v: Vec<Address> = env.storage().persistent().get(&key).unwrap_or(vec![env]);
        v.push_back(module.clone());
        env.storage().persistent().set(&key, &v);
    }
}
