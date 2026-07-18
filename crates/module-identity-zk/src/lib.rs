#![no_std]
//! ZK-backed identity provider (Phase 2) for the country predicate.
//!
//! Instead of storing a cleartext country (like `identity-mock`), the issuer registers
//! a *commitment* for an account, and the holder later submits a Groth16 proof that the
//! committed (hidden) country is in the allowed set. On success the account is marked
//! eligible. The country is never revealed on-chain.
//!
//! Same `IdentityProvider` surface as the mock (`is_verified` / `country_of`), so it is
//! a drop-in for the compliance modules — except `country_of` returns `None` (private).

use constella_module_interface::{Proof, VerificationKey};
use constella_zk_verifier::Groth16VerifierClient;
use soroban_sdk::{
    contract, contractimpl, contracttype, crypto::bls12_381::Bls12381Fr as Fr, vec, Address, Env,
    Vec, U256,
};

#[contracttype]
enum DataKey {
    Admin,
    Verifier,
    Vk,
    Allowed,
    Commitment(Address),
    Eligible(Address),
}

#[contract]
pub struct IdentityZk;

#[contractimpl]
impl IdentityZk {
    pub fn __constructor(env: Env, admin: Address, verifier: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Verifier, &verifier);
    }

    /// Admin sets the verifying key + the allowed country set (the policy).
    pub fn set_policy(env: Env, vk: VerificationKey, allowed: Vec<u32>) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Vk, &vk);
        env.storage().instance().set(&DataKey::Allowed, &allowed);
    }

    pub fn allowed(env: Env) -> Vec<u32> {
        env.storage().instance().get(&DataKey::Allowed).unwrap()
    }

    /// Issuer (admin) registers the public commitment for an account.
    pub fn register_commitment(env: Env, account: Address, commitment: U256) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Commitment(account), &commitment);
    }

    /// Demo self-attestation: the holder registers their own commitment (wallet-authed).
    /// Production keeps issuer attestation via `register_commitment`.
    pub fn register_self(env: Env, account: Address, commitment: U256) {
        account.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Commitment(account), &commitment);
    }

    /// Submit a proof that the account's committed (hidden) country is in the allowed
    /// set. Verifies the Groth16 proof against the registered commitment + policy, and
    /// marks the account eligible on success.
    pub fn prove_eligibility(env: Env, account: Address, commitment: U256, proof: Proof) -> bool {
        let registered: U256 = match env
            .storage()
            .persistent()
            .get(&DataKey::Commitment(account.clone()))
        {
            Some(c) => c,
            None => return false,
        };
        if registered != commitment {
            return false;
        }

        let vk: VerificationKey = env.storage().instance().get(&DataKey::Vk).unwrap();
        let allowed: Vec<u32> = env.storage().instance().get(&DataKey::Allowed).unwrap();
        let verifier: Address = env.storage().instance().get(&DataKey::Verifier).unwrap();

        // Public signals = [commitment, allowed[0], allowed[1], ...] (built from on-chain
        // policy so the proof is bound to exactly this commitment + allowed set).
        let mut signals = vec![&env, Fr::from_u256(commitment)];
        for c in allowed.iter() {
            signals.push_back(Fr::from_u256(U256::from_u32(&env, c)));
        }

        let ok = Groth16VerifierClient::new(&env, &verifier).verify_proof(&vk, &proof, &signals);
        if ok {
            env.storage()
                .persistent()
                .set(&DataKey::Eligible(account), &true);
        }
        ok
    }

    /// Eligibility flag (the IdentityProvider surface used by compliance modules).
    pub fn is_verified(env: Env, account: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Eligible(account))
            .unwrap_or(false)
    }

    /// Country is private under ZK — never revealed.
    pub fn country_of(_env: Env, _account: Address) -> Option<u32> {
        None
    }
}

impl IdentityZk {
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }
}

#[cfg(test)]
mod test;
