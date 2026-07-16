#!/usr/bin/env bash
# Deploy the Constella demo stack to Stellar testnet, wire modules, set identities,
# and run a real pass + revert transfer. Writes addresses to scripts/deployed.testnet.json.
#
# Usage: bash scripts/deploy-testnet.sh
# Prereqs: `stellar` CLI, contracts built (`stellar contract build`).
set -euo pipefail

NET=testnet
WASM=target/wasm32v1-none/release

echo "▸ Building contracts…"
stellar contract build >/dev/null

key() { stellar keys address "$1" 2>/dev/null; }

echo "▸ Funding identities (deployer, alice, bob)…"
for k in deployer alice bob; do
  stellar keys generate "$k" --network "$NET" --fund --overwrite >/dev/null 2>&1 || \
  stellar keys generate "$k" --network "$NET" --fund >/dev/null 2>&1 || true
done
# carol = recipient only (disallowed country); no funding needed
stellar keys generate carol --network "$NET" --overwrite >/dev/null 2>&1 || true

ADMIN=$(key deployer); ALICE=$(key alice); BOB=$(key bob); CAROL=$(key carol)
echo "  deployer=$ADMIN"; echo "  alice=$ALICE (US)"; echo "  bob=$BOB (DE)"; echo "  carol=$CAROL (TR, disallowed)"

dep() { # dep <wasm-name> <constructor-args...>
  local name="$1"; shift
  stellar contract deploy --wasm "$WASM/$name.wasm" --source deployer --network "$NET" -- "$@" 2>/dev/null | tail -1
}
inv() { # inv <contract-id> <source> <fn> <args...>
  local id="$1" src="$2"; shift 2
  stellar contract invoke --id "$id" --source "$src" --network "$NET" -- "$@" 2>/dev/null
}

echo "▸ Deploying contracts…"
IDENTITY=$(dep constella_identity_mock --admin "$ADMIN")
COMPLIANCE=$(dep constella_compliance --admin "$ADMIN")
MAX_HOLDERS=$(dep constella_module_max_holders --admin "$ADMIN" --dispatcher "$COMPLIANCE" --max 5)
LOCKUP=$(dep constella_module_lockup --admin "$ADMIN" --dispatcher "$COMPLIANCE" --lock_seconds 0)
MAX_BALANCE=$(dep constella_module_max_balance --admin "$ADMIN" --dispatcher "$COMPLIANCE" --max_per_holder 1000000)
COUNTRY=$(dep constella_module_country_restrict --admin "$ADMIN" --identity "$IDENTITY" --allowed '[840,276]')
TOKEN=$(dep constella_demo_token --admin "$ADMIN" --compliance "$COMPLIANCE")
echo "  identity=$IDENTITY"; echo "  compliance=$COMPLIANCE"; echo "  token=$TOKEN"

echo "▸ Registering modules on hooks…"
reg() { inv "$COMPLIANCE" deployer add_module_to --hook "$1" --module "$2" >/dev/null; }
for h in CanCreate CanTransfer Created Transferred Destroyed; do reg "$h" "$MAX_HOLDERS"; reg "$h" "$MAX_BALANCE"; done
for h in CanTransfer Created Transferred; do reg "$h" "$LOCKUP"; done
for h in CanCreate CanTransfer; do reg "$h" "$COUNTRY"; done

echo "▸ Attestor sets identities…"
inv "$IDENTITY" deployer set_country --account "$ALICE" --code 840 >/dev/null
inv "$IDENTITY" deployer set_country --account "$BOB"   --code 276 >/dev/null
inv "$IDENTITY" deployer set_country --account "$CAROL" --code 792 >/dev/null

echo "▸ Mint 500 to alice (compliant)…"
inv "$TOKEN" deployer mint --to "$ALICE" --amount 500 >/dev/null
echo "  alice balance = $(inv "$TOKEN" deployer balance --id "$ALICE")"

echo "▸ Transfer alice→bob 100 (should PASS)…"
inv "$TOKEN" alice transfer --from "$ALICE" --to "$BOB" --amount 100 >/dev/null && echo "  ✅ passed"
echo "  bob balance = $(inv "$TOKEN" deployer balance --id "$BOB")"

echo "▸ Transfer alice→carol 100 (should REVERT — TR not allowed)…"
if inv "$TOKEN" alice transfer --from "$ALICE" --to "$CAROL" --amount 100 >/dev/null 2>&1; then
  echo "  ❌ unexpectedly passed"
else
  echo "  ✅ reverted as expected (CountryRestrict)"
fi

echo "▸ Deploying reference compliant token with the W2/W3 modules…"
# A second, self-contained stack demonstrating Denylist + MaxInvestorsPerCountry
# + TransferWindow, kept separate from the stack above so each module's pass/revert
# is isolated.
REF_COMPLIANCE=$(dep constella_compliance --admin "$ADMIN")
DENYLIST=$(dep constella_module_denylist --admin "$ADMIN")
INVESTORS=$(dep constella_module_max_investors_per_country --admin "$ADMIN" --dispatcher "$REF_COMPLIANCE" --identity "$IDENTITY" --cap 1)
WINDOW=$(dep constella_module_transfer_window --admin "$ADMIN")
REF_TOKEN=$(dep constella_demo_token --admin "$ADMIN" --compliance "$REF_COMPLIANCE")
echo "  refCompliance=$REF_COMPLIANCE"; echo "  denylist=$DENYLIST"; echo "  investors=$INVESTORS (cap 1/country)"; echo "  window=$WINDOW"; echo "  refToken=$REF_TOKEN"

echo "▸ Registering W2/W3 modules on hooks…"
regref() { inv "$REF_COMPLIANCE" deployer add_module_to --hook "$1" --module "$2" >/dev/null; }
# Denylist + TransferWindow are pure pre-checks.
for h in CanCreate CanTransfer; do regref "$h" "$DENYLIST"; regref "$h" "$WINDOW"; done
# MaxInvestorsPerCountry keeps a balance mirror → all five hooks from genesis.
for h in CanCreate CanTransfer Created Transferred Destroyed; do regref "$h" "$INVESTORS"; done

# frank = a second US recipient (mint-only, no funding needed) used for the cap demo.
stellar keys generate frank --network "$NET" --overwrite >/dev/null 2>&1 || stellar keys generate frank --network "$NET" >/dev/null 2>&1 || true
FRANK=$(key frank)
inv "$IDENTITY" deployer set_country --account "$FRANK" --code 840 >/dev/null # US

echo "▸ Mint 500 to alice on refToken (compliant)…"
inv "$REF_TOKEN" deployer mint --to "$ALICE" --amount 500 >/dev/null && echo "  ✅ alice balance = $(inv "$REF_TOKEN" deployer balance --id "$ALICE")"

echo "▸ MaxInvestorsPerCountry: mint to frank (2nd US holder, cap 1) should REVERT…"
if inv "$REF_TOKEN" deployer mint --to "$FRANK" --amount 100 >/dev/null 2>&1; then
  echo "  ❌ unexpectedly passed"
else
  echo "  ✅ reverted as expected (US at cap; count=$(inv "$INVESTORS" deployer count --country 840))"
fi

echo "▸ Denylist: block bob, transfer alice→bob should REVERT…"
inv "$DENYLIST" deployer add_to_denylist --account "$BOB" >/dev/null
if inv "$REF_TOKEN" alice transfer --from "$ALICE" --to "$BOB" --amount 50 >/dev/null 2>&1; then
  echo "  ❌ unexpectedly passed"
else
  echo "  ✅ reverted as expected (bob denied)"
fi
inv "$DENYLIST" deployer remove_from_denylist --account "$BOB" >/dev/null

echo "▸ TransferWindow: freeze the token, mint should REVERT…"
inv "$WINDOW" deployer pause >/dev/null
if inv "$REF_TOKEN" deployer mint --to "$ALICE" --amount 10 >/dev/null 2>&1; then
  echo "  ❌ unexpectedly passed"
else
  echo "  ✅ reverted as expected (paused; is_paused=$(inv "$WINDOW" deployer is_paused))"
fi
inv "$WINDOW" deployer unpause >/dev/null
echo "  ✅ unpaused"

echo "▸ Deploying ZK (verifier + identity-zk) + policy…"
node_json() { node -e "let d='';process.stdin.on('data',c=>d+=c).on('end',()=>{const j=JSON.parse(d);console.log($1)})"; }
ZKARGS=$(cargo run --manifest-path tools/zk-encode/Cargo.toml --quiet 2>/dev/null)
VKJSON=$(printf '%s' "$ZKARGS" | node_json "JSON.stringify(j.vk)")
PROOFJSON=$(printf '%s' "$ZKARGS" | node_json "JSON.stringify(j.proof)")
COMMIT=$(printf '%s' "$ZKARGS" | node_json "j.commitment_dec")
VERIFIER=$(stellar contract deploy --wasm "$WASM/constella_zk_verifier.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)
IDZK=$(dep constella_module_identity_zk --admin "$ADMIN" --verifier "$VERIFIER")
inv "$IDZK" deployer set_policy --vk "$VKJSON" --allowed '[840,276]' >/dev/null
echo "  verifier=$VERIFIER"
echo "  identityZk=$IDZK"

echo "▸ Deploying ZK-gated token (gates on is_verified, not country) + eligible recipient…"
ZK_ELIG=$(dep constella_module_zk_eligibility --admin "$ADMIN" --identity "$IDZK")
ZK_COMPLIANCE=$(dep constella_compliance --admin "$ADMIN")
inv "$ZK_COMPLIANCE" deployer add_module_to --hook CanCreate --module "$ZK_ELIG" >/dev/null
inv "$ZK_COMPLIANCE" deployer add_module_to --hook CanTransfer --module "$ZK_ELIG" >/dev/null
ZK_TOKEN=$(dep constella_demo_token --admin "$ADMIN" --compliance "$ZK_COMPLIANCE")
stellar keys generate dave --network "$NET" --overwrite >/dev/null 2>&1 || stellar keys generate dave --network "$NET" >/dev/null 2>&1
DAVE=$(key dave)
inv "$IDZK" deployer register_commitment --account "$DAVE" --commitment "$COMMIT" >/dev/null
inv "$IDZK" deployer prove_eligibility --account "$DAVE" --commitment "$COMMIT" --proof "$PROOFJSON" >/dev/null
echo "  zkToken=$ZK_TOKEN"
echo "  dave (ZK-eligible recipient)=$DAVE"

echo "▸ Writing scripts/deployed.testnet.json…"
cat > scripts/deployed.testnet.json <<JSON
{
  "network": "testnet",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "contracts": {
    "identity": "$IDENTITY",
    "compliance": "$COMPLIANCE",
    "maxHolders": "$MAX_HOLDERS",
    "lockup": "$LOCKUP",
    "maxBalance": "$MAX_BALANCE",
    "countryRestrict": "$COUNTRY",
    "token": "$TOKEN"
  },
  "referenceToken": {
    "compliance": "$REF_COMPLIANCE",
    "denylist": "$DENYLIST",
    "maxInvestorsPerCountry": "$INVESTORS",
    "transferWindow": "$WINDOW",
    "token": "$REF_TOKEN"
  },
  "accounts": {
    "admin": "$ADMIN",
    "alice": "$ALICE",
    "bob": "$BOB",
    "carol": "$CAROL",
    "frank": "$FRANK"
  },
  "zk": {
    "verifier": "$VERIFIER",
    "identityZk": "$IDZK",
    "commitment": "$COMMIT",
    "proof": $PROOFJSON,
    "zkEligibility": "$ZK_ELIG",
    "zkCompliance": "$ZK_COMPLIANCE",
    "zkToken": "$ZK_TOKEN",
    "dave": "$DAVE"
  }
}
JSON
echo "✅ Done. Explorer: https://stellar.expert/explorer/testnet/contract/$TOKEN"
