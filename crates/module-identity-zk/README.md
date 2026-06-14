# module-identity-zk

A **privacy-preserving identity provider** (Phase 2) — same `IdentityProvider` surface as `identity-mock`, but the country stays private. An account proves its country is in the allowed set **without revealing which country**.

## Surface

- **`set_policy(vk, allowed)`** — admin sets the verification key + allowed country set.
- **`register_commitment(account, commitment)`** — issuer registers the account's public Poseidon commitment.
- **`prove_eligibility(account, commitment, proof)`** — builds `pub_signals = [commitment, allowed…]` and calls `zk-verifier.verify_proof` (cross-contract). On success, sets `eligible[account] = true`.
- **`is_verified(account) -> bool`** — the flag compliance reads.
- **`country_of(account) -> None`** — the country is **never stored**.

## Config

Constructor: `--admin <Address> --verifier <Address>` (the `zk-verifier` contract).

## Notes

This is the identity half of the privacy story: it produces a boolean `is_verified` that `module-zk-eligibility` gates on — so a recipient's country never appears on-chain. Circuit + proving live in [`zk/`](../../zk).
