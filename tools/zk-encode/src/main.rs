//! Encode the snarkjs proof + verification key (zk/data/*.json) into the byte format
//! the Soroban BLS12-381 verifier expects (arkworks uncompressed), as hex, ready to pass
//! to `stellar contract invoke`. Run from the repo root.

use ark_bls12_381::{Fq, Fq2, Fr, G1Affine, G2Affine};
use ark_serialize::CanonicalSerialize;
use core::str::FromStr;
use serde_json::Value;
use std::fs;

fn g1_hex(x: &str, y: &str) -> String {
    let p = G1Affine::new(Fq::from_str(x).unwrap(), Fq::from_str(y).unwrap());
    let mut b = vec![0u8; 96];
    p.serialize_uncompressed(&mut b[..]).unwrap();
    hex::encode(b)
}

fn g2_hex(x0: &str, x1: &str, y0: &str, y1: &str) -> String {
    let x = Fq2::new(Fq::from_str(x0).unwrap(), Fq::from_str(x1).unwrap());
    let y = Fq2::new(Fq::from_str(y0).unwrap(), Fq::from_str(y1).unwrap());
    let p = G2Affine::new(x, y);
    let mut b = vec![0u8; 192];
    p.serialize_uncompressed(&mut b[..]).unwrap();
    hex::encode(b)
}

fn fr_hex_be(dec: &str) -> String {
    let f = Fr::from_str(dec).unwrap();
    let mut le = [0u8; 32];
    f.serialize_uncompressed(&mut le[..]).unwrap();
    le.reverse();
    hex::encode(le)
}

fn s(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

fn main() {
    let vk: Value =
        serde_json::from_str(&fs::read_to_string("zk/data/verification_key.json").unwrap()).unwrap();
    let p: Value = serde_json::from_str(&fs::read_to_string("zk/data/proof.json").unwrap()).unwrap();
    let pubj: Value =
        serde_json::from_str(&fs::read_to_string("zk/data/public.json").unwrap()).unwrap();

    let proof = serde_json::json!({
        "a": g1_hex(&s(&p["pi_a"][0]), &s(&p["pi_a"][1])),
        "b": g2_hex(&s(&p["pi_b"][0][0]), &s(&p["pi_b"][0][1]), &s(&p["pi_b"][1][0]), &s(&p["pi_b"][1][1])),
        "c": g1_hex(&s(&p["pi_c"][0]), &s(&p["pi_c"][1])),
    });

    let ic: Vec<String> = vk["IC"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| g1_hex(&s(&e[0]), &s(&e[1])))
        .collect();
    let vkj = serde_json::json!({
        "alpha": g1_hex(&s(&vk["vk_alpha_1"][0]), &s(&vk["vk_alpha_1"][1])),
        "beta": g2_hex(&s(&vk["vk_beta_2"][0][0]), &s(&vk["vk_beta_2"][0][1]), &s(&vk["vk_beta_2"][1][0]), &s(&vk["vk_beta_2"][1][1])),
        "gamma": g2_hex(&s(&vk["vk_gamma_2"][0][0]), &s(&vk["vk_gamma_2"][0][1]), &s(&vk["vk_gamma_2"][1][0]), &s(&vk["vk_gamma_2"][1][1])),
        "delta": g2_hex(&s(&vk["vk_delta_2"][0][0]), &s(&vk["vk_delta_2"][0][1]), &s(&vk["vk_delta_2"][1][0]), &s(&vk["vk_delta_2"][1][1])),
        "ic": ic,
    });

    let signals: Vec<String> = pubj.as_array().unwrap().iter().map(|v| fr_hex_be(&s(v))).collect();

    println!(
        "{}",
        serde_json::json!({
            "proof": proof,
            "vk": vkj,
            "signals": signals,
            "commitment_dec": s(&pubj[0]),
        })
    );
}
