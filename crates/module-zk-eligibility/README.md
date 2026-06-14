# module-zk-eligibility

**Rule:** the recipient must be **ZK-eligible** — i.e. `is_verified(to)` is `true` on the ZK identity provider. A boolean check; **no country is ever read**.

| | |
|---|---|
| Hooks | `can_transfer`, `can_create` |
| Stateful | No |
| Identity-dependent | **Yes** — reads `is_verified(to)` from `module-identity-zk` |

## Behaviour

On a pre-check, calls `is_verified(to)` via the `IdentityClient`. A non-eligible recipient is simply denied — their country is never read or revealed.

## Config

Constructor: `--admin <Address> --identity <Address>` (the `module-identity-zk` contract).

## Read accessors

- `identity()` — the ZK identity provider address.

## Notes

This is the privacy win over `module-country-restrict`: there, a denial reveals the recipient's country; here, the denial is just "not eligible." The eligible/not-eligible boolean is **public by design** — that's what a compliance check exposes; ZK hides the *country*, not the verdict.
