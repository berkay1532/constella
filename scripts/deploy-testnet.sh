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
MAX_HOLDERS=$(dep constella_module_max_holders --admin "$ADMIN" --max 5)
LOCKUP=$(dep constella_module_lockup --admin "$ADMIN" --lock_seconds 0)
MAX_BALANCE=$(dep constella_module_max_balance --admin "$ADMIN" --max_per_holder 1000000)
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
  "accounts": {
    "admin": "$ADMIN",
    "alice": "$ALICE",
    "bob": "$BOB",
    "carol": "$CAROL"
  }
}
JSON
echo "✅ Done. Explorer: https://stellar.expert/explorer/testnet/contract/$TOKEN"
