#![no_std]
//! Constella module interface — the shared ABI between the compliance dispatcher
//! and pluggable compliance modules, plus the identity-provider boundary.
//!
//! This crate ships no deployable contract; it defines the contract types and the
//! generated clients (`ModuleClient`, `TokenClient`, `IdentityClient`) that the
//! dispatcher and modules use to call one another. Mirrors OpenZeppelin's
//! `ComplianceHook` surface so modules are portable to the OZ dispatcher.

use soroban_sdk::{contractclient, contracterror, contracttype, Address, Env, Vec};

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
