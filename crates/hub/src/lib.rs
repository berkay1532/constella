#![no_std]
//! Multi-tenant compliance hub. One instance serves every token: per-token issuer-admin,
//! a per-(token,hook) registry of shared module addresses, and a one-signature `launch`
//! that deploys the token (pointed at this hub) and wires the selected shared modules.

use constella_module_interface::{
    CountryRestrictClient, DenylistClient, IdentityZkAdminClient, LockupClient, MaxBalanceClient,
    MaxHoldersClient, MaxInvestorsClient, ModuleClient, TransferWindowClient, VerificationKey,
    ZkEligibilityClient,
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
    pub max_holders: u32,           // 0 = not selected
    pub lockup: u64,                // 0 = not selected
    pub transfer_window: bool,
    pub max_investors: u32,   // 0 = not selected
    pub zk_eligibility: bool, // true = country eligibility is proven privately (ZK), not cleartext
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
    Verifier,                 // shared Groth16 verifier for ZK eligibility
    ZkIdentityWasm,           // wasm hash of the per-token ZK identity (module-identity-zk)
    ZkVk,                     // shared Groth16 verifying key
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

    pub fn set_verifier(env: Env, verifier: Address) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::Verifier, &verifier);
    }

    pub fn set_zk_identity_wasm(env: Env, hash: BytesN<32>) {
        Self::require_platform_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::ZkIdentityWasm, &hash);
    }

    pub fn set_zk_vk(env: Env, vk: VerificationKey) {
        Self::require_platform_admin(&env);
        env.storage().instance().set(&DataKey::ZkVk, &vk);
    }

    /// Whether an account has proven ZK eligibility for a token (reads the token's ZK identity).
    pub fn is_verified(env: Env, token: Address, account: Address) -> bool {
        let id: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Identity(token))
            .unwrap();
        IdentityZkAdminClient::new(&env, &id).is_verified(&account)
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
        // Private (ZK) country eligibility: deploy a per-token ZK identity, set its policy to
        // the chosen allowed set, and gate on is_verified — the country is never revealed.
        // A ZK token's identity IS the ZK identity, so the cleartext country_restrict /
        // max_investors blocks below are skipped (mutual exclusion).
        if config.zk_eligibility {
            let zk_hash: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::ZkIdentityWasm)
                .unwrap();
            let verifier: Address = env.storage().instance().get(&DataKey::Verifier).unwrap();
            let vk: VerificationKey = env.storage().instance().get(&DataKey::ZkVk).unwrap();
            let identity = Self::deploy(&env, &zk_hash, (config.admin.clone(), verifier));
            IdentityZkAdminClient::new(&env, &identity).set_policy(&vk, &config.country_restrict);
            env.storage()
                .persistent()
                .set(&DataKey::Identity(token.clone()), &identity);
            let m = Self::module_addr(&env, "zk_eligibility");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            ZkEligibilityClient::new(&env, &m).configure(&token, &identity);
        }
        // Deploy ONE cleartext identity per token if any cleartext identity-dependent module is
        // selected, so country_restrict and max_investors share it. Skipped for ZK tokens.
        if !config.zk_eligibility
            && (!config.country_restrict.is_empty() || config.max_investors > 0)
        {
            let identity_hash: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::IdentityWasm)
                .unwrap();
            let identity = Self::deploy(&env, &identity_hash, (config.admin.clone(),));
            env.storage()
                .persistent()
                .set(&DataKey::Identity(token.clone()), &identity);
        }
        if !config.zk_eligibility && !config.country_restrict.is_empty() {
            let identity: Address = env
                .storage()
                .persistent()
                .get(&DataKey::Identity(token.clone()))
                .unwrap();
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
        if config.max_holders > 0 {
            let m = Self::module_addr(&env, "max_holders");
            for h in [
                hooks::CAN_CREATE,
                hooks::CAN_TRANSFER,
                hooks::CREATED,
                hooks::TRANSFERRED,
                hooks::DESTROYED,
            ] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            MaxHoldersClient::new(&env, &m).set_max(&token, &config.max_holders);
        }
        if config.lockup > 0 {
            let m = Self::module_addr(&env, "lockup");
            for h in [hooks::CAN_TRANSFER, hooks::CREATED, hooks::TRANSFERRED] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            LockupClient::new(&env, &m).set_duration(&token, &config.lockup);
        }
        if config.transfer_window {
            let m = Self::module_addr(&env, "transfer_window");
            for h in [hooks::CAN_CREATE, hooks::CAN_TRANSFER] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
        }
        if !config.zk_eligibility && config.max_investors > 0 {
            let identity: Address = env
                .storage()
                .persistent()
                .get(&DataKey::Identity(token.clone()))
                .unwrap();
            let m = Self::module_addr(&env, "max_investors");
            for h in [
                hooks::CAN_CREATE,
                hooks::CAN_TRANSFER,
                hooks::CREATED,
                hooks::TRANSFERRED,
                hooks::DESTROYED,
            ] {
                Self::register(&env, &token, &Symbol::new(&env, h), &m);
            }
            MaxInvestorsClient::new(&env, &m).configure(&token, &identity, &config.max_investors);
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
    pub fn set_max_holders(env: Env, token: Address, cap: u32) {
        Self::require_token_admin(&env, &token);
        MaxHoldersClient::new(&env, &Self::module_addr(&env, "max_holders")).set_max(&token, &cap);
    }
    pub fn max_holders(env: Env, token: Address) -> u32 {
        MaxHoldersClient::new(&env, &Self::module_addr(&env, "max_holders")).max(&token)
    }
    pub fn holders(env: Env, token: Address) -> u32 {
        MaxHoldersClient::new(&env, &Self::module_addr(&env, "max_holders")).holders(&token)
    }
    pub fn set_lockup(env: Env, token: Address, secs: u64) {
        Self::require_token_admin(&env, &token);
        LockupClient::new(&env, &Self::module_addr(&env, "lockup")).set_duration(&token, &secs);
    }
    pub fn unlock_at(env: Env, token: Address, holder: Address) -> u64 {
        LockupClient::new(&env, &Self::module_addr(&env, "lockup")).unlock_at(&token, &holder)
    }
    pub fn pause(env: Env, token: Address) {
        Self::require_token_admin(&env, &token);
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).pause(&token);
    }
    pub fn unpause(env: Env, token: Address) {
        Self::require_token_admin(&env, &token);
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window"))
            .unpause(&token);
    }
    pub fn set_window(env: Env, token: Address, open_from: Option<u64>, open_until: Option<u64>) {
        Self::require_token_admin(&env, &token);
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).set_window(
            &token,
            &open_from,
            &open_until,
        );
    }
    pub fn is_paused(env: Env, token: Address) -> bool {
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window"))
            .is_paused(&token)
    }
    pub fn transfer_window(env: Env, token: Address) -> (Option<u64>, Option<u64>) {
        TransferWindowClient::new(&env, &Self::module_addr(&env, "transfer_window")).window(&token)
    }
    pub fn set_investor_cap(env: Env, token: Address, cap: u32) {
        Self::require_token_admin(&env, &token);
        MaxInvestorsClient::new(&env, &Self::module_addr(&env, "max_investors"))
            .set_cap(&token, &cap);
    }
    pub fn investor_cap(env: Env, token: Address) -> u32 {
        MaxInvestorsClient::new(&env, &Self::module_addr(&env, "max_investors")).cap(&token)
    }
    pub fn investor_count(env: Env, token: Address, country: u32) -> u32 {
        MaxInvestorsClient::new(&env, &Self::module_addr(&env, "max_investors"))
            .count(&token, &country)
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
