#!/usr/bin/env bash
# One-time platform bootstrap: deploy the shared multi-tenant Hub + all 7 modules to testnet,
# wire the token/identity wasm + module addresses, and emit web/src/hub.testnet.json.
# The platform-admin (deployer) key signs ONLY this script — never anything in the browser.
#
# NOTE: written with parallel indexed arrays (not `declare -A`) because macOS ships bash 3.2,
# which has no associative-array support. Logic/output are otherwise identical to the spec.
set -euo pipefail
# Force a plain locale: macOS bash 3.2 combined with a non-C locale (e.g. tr_TR.UTF-8) has a
# known lexer bug where multibyte characters (▸, …) adjacent to a variable expansion corrupt
# the variable name, producing spurious "unbound variable" errors. This is cosmetic-echo-only;
# it does not affect the deployed addresses or the emitted JSON.
export LC_ALL=C
cd "$(dirname "$0")/.."
NET=testnet
W=target/wasm32v1-none/release
OUT=web/src/hub.testnet.json

echo "▸ Building wasm…"; cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
DEP=$(stellar keys address deployer)

echo "▸ Uploading token + identity wasm…"
TOKHASH=$(stellar contract upload --wasm "$W/constella_demo_token.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)
IDHASH=$(stellar contract upload --wasm "$W/constella_identity_mock.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)

echo "▸ Deploying hub…"
HUB=$(stellar contract deploy --wasm "$W/constella_hub.wasm" --source deployer --network "$NET" -- --platform_admin "$DEP" 2>/dev/null | tail -1)

KINDS=(denylist max_balance country_restrict max_holders lockup transfer_window max_investors)
WASMS=(constella_hub_module_denylist constella_hub_module_max_balance constella_hub_module_country_restrict constella_hub_module_max_holders constella_hub_module_lockup constella_hub_module_transfer_window constella_hub_module_max_investors_per_country)
ADDRS=()

for i in "${!KINDS[@]}"; do
  kind="${KINDS[$i]}"
  wasm="${WASMS[$i]}"
  echo "▸ Deploying module $kind…"
  addr=$(stellar contract deploy --wasm "$W/${wasm}.wasm" --source deployer --network "$NET" -- --hub "$HUB" 2>/dev/null | tail -1)
  ADDRS+=("$addr")
done

echo "▸ Platform config on hub…"
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_token_wasm --hash "$TOKHASH" >/dev/null
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_identity_wasm --hash "$IDHASH" >/dev/null
for i in "${!KINDS[@]}"; do
  stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_module_addr --kind "${KINDS[$i]}" --addr "${ADDRS[$i]}" >/dev/null
done

# helper to look up an address by kind name
addr_for() {
  local want="$1"
  for i in "${!KINDS[@]}"; do
    if [ "${KINDS[$i]}" = "$want" ]; then
      echo "${ADDRS[$i]}"
      return 0
    fi
  done
}

echo "▸ Writing $OUT…"
cat > "$OUT" <<JSON
{
  "network": "testnet",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "hub": "$HUB",
  "modules": {
    "denylist": "$(addr_for denylist)",
    "max_balance": "$(addr_for max_balance)",
    "country_restrict": "$(addr_for country_restrict)",
    "max_holders": "$(addr_for max_holders)",
    "lockup": "$(addr_for lockup)",
    "transfer_window": "$(addr_for transfer_window)",
    "max_investors": "$(addr_for max_investors)"
  }
}
JSON
echo "=== BOOTSTRAP DONE ==="; cat "$OUT"
