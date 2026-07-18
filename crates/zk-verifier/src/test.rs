#![cfg(test)]
extern crate std;

use core::str::FromStr;
use std::string::ToString;
use std::vec::Vec as StdVec;

use ark_bls12_381::{Fq, Fq2};
use ark_serialize::CanonicalSerialize;
use serde_json::Value;
use soroban_sdk::{
    crypto::bls12_381::{Fr, G1Affine, G2Affine, G1_SERIALIZED_SIZE, G2_SERIALIZED_SIZE},
    Bytes, Env, Vec, U256,
};

use crate::{Groth16Verifier, Groth16VerifierClient};
use constella_module_interface::{Proof, VerificationKey};

const PROOF: &str = include_str!("../../../zk/data/proof.json");
const VK: &str = include_str!("../../../zk/data/verification_key.json");
const PUBLIC: &str = include_str!("../../../zk/data/public.json");

fn s(v: &Value) -> std::string::String {
    v.as_str().unwrap().to_string()
}

fn g1(env: &Env, x: &str, y: &str) -> G1Affine {
    let p = ark_bls12_381::G1Affine::new(Fq::from_str(x).unwrap(), Fq::from_str(y).unwrap());
    let mut buf = [0u8; G1_SERIALIZED_SIZE];
    p.serialize_uncompressed(&mut buf[..]).unwrap();
    G1Affine::from_array(env, &buf)
}

fn g2(env: &Env, x0: &str, x1: &str, y0: &str, y1: &str) -> G2Affine {
    let x = Fq2::new(Fq::from_str(x0).unwrap(), Fq::from_str(x1).unwrap());
    let y = Fq2::new(Fq::from_str(y0).unwrap(), Fq::from_str(y1).unwrap());
    let p = ark_bls12_381::G2Affine::new(x, y);
    let mut buf = [0u8; G2_SERIALIZED_SIZE];
    p.serialize_uncompressed(&mut buf[..]).unwrap();
    G2Affine::from_array(env, &buf)
}

fn fr(env: &Env, dec: &str) -> Fr {
    let f = ark_bls12_381::Fr::from_str(dec).unwrap();
    let mut le = [0u8; 32];
    f.serialize_uncompressed(&mut le[..]).unwrap(); // little-endian
    le.reverse(); // -> big-endian
    Fr::from_u256(U256::from_be_bytes(env, &Bytes::from_array(env, &le)))
}

fn load(env: &Env) -> (VerificationKey, Proof, Vec<Fr>) {
    let vk_j: Value = serde_json::from_str(VK).unwrap();
    let p_j: Value = serde_json::from_str(PROOF).unwrap();
    let pub_j: Value = serde_json::from_str(PUBLIC).unwrap();

    let mut ic = Vec::new(env);
    for e in vk_j["IC"].as_array().unwrap() {
        ic.push_back(g1(env, &s(&e[0]), &s(&e[1])));
    }
    let vk = VerificationKey {
        alpha: g1(env, &s(&vk_j["vk_alpha_1"][0]), &s(&vk_j["vk_alpha_1"][1])),
        beta: g2(env, &s(&vk_j["vk_beta_2"][0][0]), &s(&vk_j["vk_beta_2"][0][1]), &s(&vk_j["vk_beta_2"][1][0]), &s(&vk_j["vk_beta_2"][1][1])),
        gamma: g2(env, &s(&vk_j["vk_gamma_2"][0][0]), &s(&vk_j["vk_gamma_2"][0][1]), &s(&vk_j["vk_gamma_2"][1][0]), &s(&vk_j["vk_gamma_2"][1][1])),
        delta: g2(env, &s(&vk_j["vk_delta_2"][0][0]), &s(&vk_j["vk_delta_2"][0][1]), &s(&vk_j["vk_delta_2"][1][0]), &s(&vk_j["vk_delta_2"][1][1])),
        ic,
    };
    let proof = Proof {
        a: g1(env, &s(&p_j["pi_a"][0]), &s(&p_j["pi_a"][1])),
        b: g2(env, &s(&p_j["pi_b"][0][0]), &s(&p_j["pi_b"][0][1]), &s(&p_j["pi_b"][1][0]), &s(&p_j["pi_b"][1][1])),
        c: g1(env, &s(&p_j["pi_c"][0]), &s(&p_j["pi_c"][1])),
    };
    let signals_dec: StdVec<std::string::String> =
        pub_j.as_array().unwrap().iter().map(|v| s(v)).collect();
    let mut signals = Vec::new(env);
    for d in &signals_dec {
        signals.push_back(fr(env, d));
    }
    (vk, proof, signals)
}

#[test]
fn verifies_real_proof() {
    let env = Env::default();
    let (vk, proof, signals) = load(&env);
    let client = Groth16VerifierClient::new(&env, &env.register(Groth16Verifier {}, ()));
    assert_eq!(client.verify_proof(&vk, &proof, &signals), true);
}

#[test]
fn rejects_tampered_public_signal() {
    let env = Env::default();
    let (vk, proof, mut signals) = load(&env);
    // Flip the first public signal (the commitment) -> proof must no longer verify.
    signals.set(0, fr(&env, "12345"));
    let client = Groth16VerifierClient::new(&env, &env.register(Groth16Verifier {}, ()));
    assert_eq!(client.verify_proof(&vk, &proof, &signals), false);
}
