# zk-verifier

A **Groth16 verifier** over **BLS12-381**, on-chain (Phase 2). Adapted from `stellar/soroban-examples/groth16_verifier`.

## Surface

- **`verify_proof(vk, proof, pub_signals) -> bool`** — verifies a Groth16 proof against a verification key and public signals using `env.crypto().bls12_381().pairing_check`:
  `e(-A, B) · e(α, β) · e(vk_x, γ) · e(C, δ) == 1`.

## Types

- `VerificationKey { alpha: G1, beta/gamma/delta: G2, ic: Vec<G1> }`
- `Proof { a: G1, b: G2, c: G1 }`
- Public signals are `Vec<Fr>` (decimal u256).

## Notes

- Encoding: G1 = 96 bytes, G2 = 192 bytes (uncompressed). Off-chain conversion from snarkjs JSON lives in `tools/zk-encode`.
- Cost: the pairing check is ~40M instructions (~40% of the 100M tx budget).
- **BLS12-381 is the right, required curve** (Soroban's on-chain crypto supports it; 128-bit security). The demo-grade caveat is the *hash* (circomlib Poseidon ships BN254-tuned constants), not the curve. See the [root README](../../README.md) honest-caveats section.
