# module-country-restrict

**Rule:** the recipient's attested country must be in an allowed set.

| | |
|---|---|
| Hooks | `can_transfer`, `can_create` |
| Stateful | No |
| Identity-dependent | **Yes** — reads `country_of(to)` from the identity provider |

## Behaviour

On a pre-check, reads `country_of(to)` via the `IdentityClient`. If the country is unknown (`None`) or not in the allowed list, returns `false` (deny). Otherwise `true`.

## Config

Constructor: `--admin <Address> --identity <Address> --allowed '[840,276]'` (ISO-3166 numeric codes; `840`=US, `276`=DE).

## Read accessors

- `allowed()` — the allowed country list.
- `identity()` — the identity provider address.

## Notes

Because it reads a cleartext `country_of`, a denial **reveals** the recipient's country on-chain. The ZK-gated alternative (`module-zk-eligibility`) avoids this by checking a boolean eligibility flag instead.
