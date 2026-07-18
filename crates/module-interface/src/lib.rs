#![no_std]
//! Constella module interface — the shared ABI between the compliance dispatcher
//! and pluggable compliance modules, plus the identity-provider boundary.
//!
//! This crate ships no deployable contract; it defines the contract types and the
//! generated clients (`ModuleClient`, `TokenClient`, `IdentityClient`) that the
//! dispatcher and modules use to call one another. Mirrors OpenZeppelin's
//! `ComplianceHook` surface so modules are portable to the OZ dispatcher.

use soroban_sdk::crypto::bls12_381::{Bls12381G1Affine as G1Affine, Bls12381G2Affine as G2Affine};
use soroban_sdk::{contractclient, contracterror, contracttype, Address, Env, Vec};

/// Groth16 (BLS12-381) verification key. Shared here (rather than in the `zk-verifier`
/// `#[contract]` crate) so non-verifier crates — the hub, the ZK identity — can carry it
/// without depending on a `#[contract]`. Layout is identical to the original definition.
#[derive(Clone)]
#[contracttype]
pub struct VerificationKey {
    pub alpha: G1Affine,
    pub beta: G2Affine,
    pub gamma: G2Affine,
    pub delta: G2Affine,
    pub ic: Vec<G1Affine>,
}

/// Groth16 proof (BLS12-381). Shared alongside [`VerificationKey`].
#[derive(Clone)]
#[contracttype]
pub struct Proof {
    pub a: G1Affine,
    pub b: G2Affine,
    pub c: G1Affine,
}

/// Hook points the compliance dispatcher exposes. A module registers against one
/// or more hooks; the dispatcher only invokes a module on the hooks it registered for.
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComplianceHook {
    /// Pre-check before a transfer; all registered modules must return `true`.
    CanTransfer,
    /// Pre-check before a mint/create.
    CanCreate,
    /// Post-event after a transfer settles (state updates).
    Transferred,
    /// Post-event after a mint/create.
    Created,
    /// Post-event after a burn/destroy.
    Destroyed,
}

/// Shared error space for compliance components.
#[contracterror]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum ComplianceError {
    NotAuthorized = 1,
    AlreadyInitialized = 2,
    NotInitialized = 3,
    ModuleAlreadyRegistered = 4,
    ModuleNotRegistered = 5,
    /// A pre-check denied the operation (used by the demo token to surface a typed revert).
    Denied = 6,
}

/// The interface every compliance module conforms to. A module implements only the
/// hooks relevant to it (e.g. a stateless check implements just `can_transfer`).
/// The dispatcher calls these through the generated [`ModuleClient`].
#[contractclient(name = "ModuleClient")]
pub trait ComplianceModule {
    /// Return `true` if the transfer is allowed by this module's rule.
    fn can_transfer(env: Env, from: Address, to: Address, amount: i128, token: Address) -> bool;
    /// Return `true` if the mint/create is allowed by this module's rule.
    fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool;
    /// Post-settlement hook for stateful modules to update their bookkeeping.
    fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address);
    /// Post-mint hook.
    fn created(env: Env, to: Address, amount: i128, token: Address);
    /// Post-burn hook.
    fn destroyed(env: Env, from: Address, amount: i128, token: Address);
}

/// Minimal SEP-41 read surface used by balance-dependent modules.
#[contractclient(name = "TokenClient")]
pub trait TokenRead {
    fn balance(env: Env, id: Address) -> i128;
}

/// Identity/attribute provider — the attestor boundary. MVP implementation is a
/// mock registry; Phase 2 is a ZK-backed provider. Modules depend only on this
/// interface, never on the implementation.
#[contractclient(name = "IdentityClient")]
pub trait IdentityProvider {
    /// ISO-3166 numeric country code for an account, if known/attested.
    fn country_of(env: Env, account: Address) -> Option<u32>;
    /// Whether the account has been verified by the attestor.
    fn is_verified(env: Env, account: Address) -> bool;
}

/// Admin surface of the multi-tenant denylist module, called by the hub's forwarders.
/// Token-keyed so one shared instance serves every token.
#[contractclient(name = "DenylistClient")]
pub trait DenylistAdmin {
    fn add_to_denylist(env: Env, token: Address, account: Address);
    fn remove_from_denylist(env: Env, token: Address, account: Address);
    fn is_denied(env: Env, token: Address, account: Address) -> bool;
}

/// Config surface of the multi-tenant MaxBalance module, called by the hub (launch init
/// + the issuer forwarder). Token-keyed.
#[contractclient(name = "MaxBalanceClient")]
pub trait MaxBalanceAdmin {
    fn set_max(env: Env, token: Address, cap: i128);
    fn max(env: Env, token: Address) -> i128;
}

/// Config surface of the multi-tenant CountryRestrict module, called by the hub. Token-keyed.
#[contractclient(name = "CountryRestrictClient")]
pub trait CountryRestrictAdmin {
    fn configure(env: Env, token: Address, identity: Address, allowed: Vec<u32>);
    fn set_allowed(env: Env, token: Address, allowed: Vec<u32>);
    fn allowed(env: Env, token: Address) -> Vec<u32>;
    fn identity(env: Env, token: Address) -> Address;
}

/// Config surface of the multi-tenant MaxHolders module, called by the hub. Token-keyed.
#[contractclient(name = "MaxHoldersClient")]
pub trait MaxHoldersAdmin {
    fn set_max(env: Env, token: Address, cap: u32);
    fn max(env: Env, token: Address) -> u32;
    fn holders(env: Env, token: Address) -> u32;
}

/// Config surface of the multi-tenant Lockup module, called by the hub. Token-keyed.
#[contractclient(name = "LockupClient")]
pub trait LockupAdmin {
    fn set_duration(env: Env, token: Address, secs: u64);
    fn unlock_at(env: Env, token: Address, holder: Address) -> u64;
}

/// Config surface of the multi-tenant TransferWindow module, called by the hub. Token-keyed.
#[contractclient(name = "TransferWindowClient")]
pub trait TransferWindowAdmin {
    fn pause(env: Env, token: Address);
    fn unpause(env: Env, token: Address);
    fn set_window(env: Env, token: Address, open_from: Option<u64>, open_until: Option<u64>);
    fn is_paused(env: Env, token: Address) -> bool;
    fn window(env: Env, token: Address) -> (Option<u64>, Option<u64>);
}

/// Config surface of the multi-tenant MaxInvestorsPerCountry module, called by the hub. Token-keyed.
#[contractclient(name = "MaxInvestorsClient")]
pub trait MaxInvestorsAdmin {
    fn configure(env: Env, token: Address, identity: Address, cap: u32);
    fn set_cap(env: Env, token: Address, cap: u32);
    fn cap(env: Env, token: Address) -> u32;
    fn count(env: Env, token: Address, country: u32) -> u32;
}

/// Config surface of the multi-tenant ZkEligibility module, called by the hub. Token-keyed.
#[contractclient(name = "ZkEligibilityClient")]
pub trait ZkEligibilityAdmin {
    fn configure(env: Env, token: Address, identity: Address);
    fn identity(env: Env, token: Address) -> Address;
}

/// Policy surface of the per-token ZK identity (`module-identity-zk`), called by the hub at
/// launch to set the token's Groth16 verifying key + allowed-country set, and read eligibility.
#[contractclient(name = "IdentityZkAdminClient")]
pub trait IdentityZkAdmin {
    fn set_policy(env: Env, vk: VerificationKey, allowed: Vec<u32>);
    fn is_verified(env: Env, account: Address) -> bool;
}
