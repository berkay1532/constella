# Constella — Web demo (Freighter, real transactions)

A React + Vite app that talks to the Constella contracts on Stellar testnet. Connect
**Freighter**, and your wallet becomes a verified holder of the regulated token: you can
send a real, signed transfer to **Bob (DE — allowed)** but the **CountryRestrict** module
blocks a transfer to **Carol (TR — not allowed)** before you even sign.

## Run

```bash
cd web
npm install
npm run dev      # open the printed localhost URL
```

Requirements:
- The **Freighter** browser extension, set to **Testnet**.
- The local `stellar` CLI with a funded **`deployer`** identity (the same one used by
  `../scripts/deploy-testnet.sh`). The dev server exposes a small `/api/bootstrap`
  endpoint that uses it to fund + attest + mint to your connected wallet. The admin
  secret never enters the frontend bundle, and this endpoint only exists under `npm run dev`.

## Flow

1. **Connect Freighter** — shows your address.
2. **Prepare my wallet** — funds it (friendbot), attests its country (US), mints 1,000 TOK
   (admin-signed, server-side via the CLI).
3. **Send 100 → Bob** — prepares, you sign in Freighter, it submits; balances update and a
   tx link appears.
4. **Send 100 → Carol** — rejected by `CountryRestrict` at preparation; no signature needed.

It reads `src/deployed.testnet.json` (produced by `../scripts/deploy-testnet.sh`). Re-run
that script and copy the JSON to point at a fresh deployment:

```bash
bash ../scripts/deploy-testnet.sh && cp ../scripts/deployed.testnet.json src/deployed.testnet.json
```

> A read-only **simulation** path also exists in `src/stellar.ts` (no wallet needed) if you
> want to show the gate without signing.
