# Constella — Web demo (launch-taste)

A minimal React + Vite app that talks to the Constella contracts deployed on Stellar
testnet. It shows the compliant token, its registered compliance modules, attested
investors, and lets you **live-simulate a transfer** to see the modules allow or deny
it in real time (via Soroban RPC `simulateTransaction`).

## Run

```bash
cd web
npm install
npm run dev      # open the printed localhost URL
```

The app reads `src/deployed.testnet.json` (contract ids + accounts), produced by
`../scripts/deploy-testnet.sh`. To point at a fresh deployment, re-run that script and
copy the JSON:

```bash
bash ../scripts/deploy-testnet.sh
cp ../scripts/deployed.testnet.json src/deployed.testnet.json
```

## What it demonstrates

- **Alice (US) → Bob (DE)**: allowed by all modules.
- **Alice (US) → Carol (TR)**: denied by `CountryRestrict`.

Transfers are *simulated* (read-only) so no wallet/signing is needed for the demo. In
production the sender signs with a wallet (e.g. Freighter) and the same modules gate
the real transaction.
