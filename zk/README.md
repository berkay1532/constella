# Constella ZK — country eligibility (Phase 2)

Proves that an investor's **country is in an allowed set without revealing the country**,
verified on-chain by a Groth16/BLS12-381 verifier (Soroban).

## Statement

Private: `country`, `secret`. Public: `commitment` (circuit output), `allowed[]`.

The circuit (`circuits/country_eligibility.circom`) proves:
1. `commitment == Poseidon(country, secret)` — binds to an issuer-registered commitment.
2. `country ∈ allowed` — via `∏(country − allowed[i]) == 0` (no hash needed).

So a holder can prove eligibility against an issuer's commitment while the country stays
private. The on-chain flow lives in `crates/module-identity-zk` (registers commitments,
verifies proofs via `crates/zk-verifier`, sets an eligibility flag) — a drop-in
`IdentityProvider` whose `country_of` returns `None`.

## Regenerate the proof artifacts

```bash
cd zk
npm install          # snarkjs + circomlib
bash build.sh        # circom compile + BLS12-381 Groth16 setup + prove + off-chain verify
```

Outputs `data/{proof,public,verification_key}.json` — consumed by the Rust tests via
`include_str!`. Build artifacts (ptau/zkey/r1cs/wasm/witness) go to `build/` (gitignored).

## ⚠️ Demonstration only

Not audited. Notes:
- circomlib Poseidon constants are BN254-derived; over BLS12-381 this is a non-standard
  (demo-grade) parameterization, not a production hash.
- The trusted setup (powers of tau + zkey) here is a single local contribution, not a
  real ceremony.
- Binding a *real* KYC credential (issuer signature) into the circuit is future work; the
  demo uses a Poseidon commitment registered by the issuer.
