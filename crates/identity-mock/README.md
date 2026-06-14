# identity-mock

A **cleartext attestor** — the MVP implementation of the `IdentityProvider` boundary. Stores attested attributes in the clear (no privacy). For the privacy-preserving variant see `module-identity-zk`.

## Surface

- **`set_country(account, code)`** — attestor sets an account's ISO-3166 numeric country code (admin).
- **`set_verified(account, flag)`** — mark an account verified (admin).
- **`country_of(account) -> Option<u32>`** — read the attested country.
- **`is_verified(account) -> bool`** — read the verification flag.

## Config

Constructor: `--admin <Address>` (the attestor).

## Notes

`country_of` returns the country **in the clear** — this is exactly the privacy gap the ZK layer closes (`module-identity-zk` returns `country_of -> None`). Identity-dependent modules like `module-country-restrict` read this provider through the `IdentityClient`.
