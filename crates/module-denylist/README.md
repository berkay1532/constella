# module-denylist

**Rule:** sanctioned addresses may neither send nor receive (admin-managed blocklist).

| | |
|---|---|
| Hooks | `can_transfer`, `can_create` |
| Stateful | **Yes** — self-held denylist set |
| Identity-dependent | No |

## Behaviour

The admin maintains a blocklist of addresses. A transfer is denied if **either** `from` or `to` is on the list; a mint is denied if `to` is on the list. The module holds its own set and never reads the token or an identity provider, so there is no re-entrancy concern and no post-event bookkeeping.

## Config

Constructor: `--admin <Address>`.

## Admin operations

- `add_to_denylist(account)` — block an address. Admin-only.
- `remove_from_denylist(account)` — unblock an address. Admin-only.

## Read accessors

- `is_denied(account)` — whether an address is currently blocked.

## Notes

Only the pre-check hooks (`can_transfer`, `can_create`) are used — the rule is a pure set-membership check, so no genesis-registration constraint applies (unlike the balance-mirror modules). See [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
