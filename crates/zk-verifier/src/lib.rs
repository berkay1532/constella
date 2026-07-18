#![no_std]
//! Groth16 (BLS12-381) proof verifier as a Soroban contract.
//!
//! Uses the SDK's native BLS12-381 crypto (`env.crypto().bls12_381()`), so a single
//! verification fits well within the per-transaction budget (~40M instructions).
//! Adapted from the official `stellar/soroban-examples/groth16_verifier`.
//!
//! Constella uses this to verify a proof that an investor's (hidden) country is in an
//! allowed set and matches an issuer-registered commitment — see `module-identity-zk`.

use constella_module_interface::{Proof, VerificationKey};
use soroban_sdk::{
    contract, contracterror, contractimpl,
    crypto::bls12_381::Bls12381Fr as Fr,
    vec, Env, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Groth16Error {
    MalformedVerifyingKey = 0,
}

#[contract]
pub struct Groth16Verifier;

#[contractimpl]
impl Groth16Verifier {
    /// Verify a Groth16 proof for the given verification key and public signals.
    pub fn verify_proof(
        env: Env,
        vk: VerificationKey,
        proof: Proof,
        pub_signals: Vec<Fr>,
    ) -> Result<bool, Groth16Error> {
        let bls = env.crypto().bls12_381();

        // vk_x = ic[0] + sum_i pub_signals[i] * ic[i+1]
        if pub_signals.len() + 1 != vk.ic.len() {
            return Err(Groth16Error::MalformedVerifyingKey);
        }
        let mut vk_x = vk.ic.get(0).unwrap();
        for (s, v) in pub_signals.iter().zip(vk.ic.iter().skip(1)) {
            let prod = bls.g1_mul(&v, &s);
            vk_x = bls.g1_add(&vk_x, &prod);
        }

        // e(-A, B) * e(alpha, beta) * e(vk_x, gamma) * e(C, delta) == 1
        let neg_a = -proof.a;
        let vp1 = vec![&env, neg_a, vk.alpha, vk_x, proof.c];
        let vp2 = vec![&env, proof.b, vk.beta, vk.gamma, vk.delta];

        Ok(bls.pairing_check(vp1, vp2))
    }
}

#[cfg(test)]
mod test;
