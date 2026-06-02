#![cfg(test)]
extern crate std;

use core::str::FromStr;
use std::string::{String, ToString};

use ark_bls12_381::{Fq, Fq2};
use ark_serialize::CanonicalSerialize;
use serde_json::Value;
use soroban_sdk::{
    crypto::bls12_381::{G1Affine, G2Affine, G1_SERIALIZED_SIZE, G2_SERIALIZED_SIZE},
    testutils::Address as _,
    Address, Bytes, Env, Vec, U256,
};

use constella_zk_verifier::{Groth16Verifier, Proof, VerificationKey};

use crate::{IdentityZk, IdentityZkClient};

const PROOF: &str = include_str!("../../../zk/data/proof.json");
const VK: &str = include_str!("../../../zk/data/verification_key.json");
const PUBLIC: &str = include_str!("../../../zk/data/public.json");

fn s(v: &Value) -> String {
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

fn u256_from_dec(env: &Env, dec: &str) -> U256 {
    let f = ark_bls12_381::Fr::from_str(dec).unwrap();
    let mut le = [0u8; 32];
    f.serialize_uncompressed(&mut le[..]).unwrap();
    le.reverse();
    U256::from_be_bytes(env, &Bytes::from_array(env, &le))
}

fn vk(env: &Env) -> VerificationKey {
    let j: Value = serde_json::from_str(VK).unwrap();
    let mut ic = Vec::new(env);
    for e in j["IC"].as_array().unwrap() {
        ic.push_back(g1(env, &s(&e[0]), &s(&e[1])));
    }
    VerificationKey {
        alpha: g1(env, &s(&j["vk_alpha_1"][0]), &s(&j["vk_alpha_1"][1])),
        beta: g2(env, &s(&j["vk_beta_2"][0][0]), &s(&j["vk_beta_2"][0][1]), &s(&j["vk_beta_2"][1][0]), &s(&j["vk_beta_2"][1][1])),
        gamma: g2(env, &s(&j["vk_gamma_2"][0][0]), &s(&j["vk_gamma_2"][0][1]), &s(&j["vk_gamma_2"][1][0]), &s(&j["vk_gamma_2"][1][1])),
        delta: g2(env, &s(&j["vk_delta_2"][0][0]), &s(&j["vk_delta_2"][0][1]), &s(&j["vk_delta_2"][1][0]), &s(&j["vk_delta_2"][1][1])),
        ic,
    }
}

fn proof(env: &Env) -> Proof {
    let j: Value = serde_json::from_str(PROOF).unwrap();
    Proof {
        a: g1(env, &s(&j["pi_a"][0]), &s(&j["pi_a"][1])),
        b: g2(env, &s(&j["pi_b"][0][0]), &s(&j["pi_b"][0][1]), &s(&j["pi_b"][1][0]), &s(&j["pi_b"][1][1])),
        c: g1(env, &s(&j["pi_c"][0]), &s(&j["pi_c"][1])),
    }
}

fn commitment(env: &Env) -> U256 {
    let pub_j: Value = serde_json::from_str(PUBLIC).unwrap();
    u256_from_dec(env, &s(&pub_j[0]))
}

fn setup(env: &Env) -> (IdentityZkClient<'static>, Address) {
    let admin = Address::generate(env);
    let verifier = env.register(Groth16Verifier {}, ());
    let id = env.register(IdentityZk, (admin.clone(), verifier));
    let client = IdentityZkClient::new(env, &id);
    client.set_policy(&vk(env), &Vec::from_array(env, [840u32, 276u32]));
    let investor = Address::generate(env);
    (client, investor)
}

#[test]
fn proves_eligibility_with_hidden_country() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = setup(&env);

    // Issuer registers the investor's commitment; investor then proves eligibility.
    client.register_commitment(&investor, &commitment(&env));
    assert_eq!(client.is_verified(&investor), false);

    let ok = client.prove_eligibility(&investor, &commitment(&env), &proof(&env));
    assert_eq!(ok, true);
    assert_eq!(client.is_verified(&investor), true);
    // Country stays private.
    assert_eq!(client.country_of(&investor), None);
}

#[test]
fn rejects_unregistered_or_wrong_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = setup(&env);

    // No commitment registered yet -> rejected.
    assert_eq!(client.prove_eligibility(&investor, &commitment(&env), &proof(&env)), false);

    // Registered, but caller claims a different commitment -> rejected.
    client.register_commitment(&investor, &commitment(&env));
    let wrong = U256::from_u32(&env, 12345);
    assert_eq!(client.prove_eligibility(&investor, &wrong, &proof(&env)), false);
    assert_eq!(client.is_verified(&investor), false);
}
