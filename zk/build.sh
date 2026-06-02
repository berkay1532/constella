#!/usr/bin/env bash
# Compile the country-eligibility circuit over BLS12-381 and produce a real Groth16
# proof + verification key (snarkjs). Outputs land in zk/build/ and zk/data/.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$PWD/node_modules/.bin:$PATH"

NAME=country_eligibility
mkdir -p build data
echo "▸ circom compile (BLS12-381)…"
circom "circuits/$NAME.circom" --r1cs --wasm --prime bls12381 -l node_modules -o build >/dev/null
snarkjs r1cs info "build/$NAME.r1cs"

echo "▸ powers of tau (bls12-381)…"
snarkjs powersoftau new bls12-381 14 build/pot_0.ptau >/dev/null 2>&1
snarkjs powersoftau contribute build/pot_0.ptau build/pot_1.ptau --name="c1" -e="constella-entropy-1" >/dev/null 2>&1
snarkjs powersoftau prepare phase2 build/pot_1.ptau build/pot_final.ptau >/dev/null 2>&1

echo "▸ groth16 setup…"
snarkjs groth16 setup "build/$NAME.r1cs" build/pot_final.ptau build/${NAME}_0.zkey >/dev/null 2>&1
snarkjs zkey contribute build/${NAME}_0.zkey build/${NAME}_final.zkey --name="c1" -e="constella-entropy-2" >/dev/null 2>&1
snarkjs zkey export verificationkey build/${NAME}_final.zkey data/verification_key.json >/dev/null 2>&1

echo "▸ witness + prove…"
node "build/${NAME}_js/generate_witness.js" "build/${NAME}_js/$NAME.wasm" input.json build/witness.wtns >/dev/null
snarkjs groth16 prove build/${NAME}_final.zkey build/witness.wtns data/proof.json data/public.json >/dev/null

echo "▸ off-chain verify…"
snarkjs groth16 verify data/verification_key.json data/public.json data/proof.json

echo "▸ public signals (commitment, allowed…):"
cat data/public.json
