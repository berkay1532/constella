# module-lockup

**Rule:** an acquired balance cannot be transferred until **T seconds** after it was acquired (lock-up / holding period).

| | |
|---|---|
| Hooks | `can_transfer`, `created`, `transferred` |
| Stateful | **Yes** — per-holder acquisition timestamp |
| Identity-dependent | No |

## Behaviour

On `created`/`transferred` (to `to`), records the acquisition ledger time. On a transfer pre-check from `from`, allows only if `now >= acquired[from] + T`.

## Config

Constructor: `--admin <Address> --lock_seconds <u64>` (`0` in the demo for instant transfers).

## Read accessors

- `duration()` — the lock-up period in seconds.
- `unlock_at(account)` — the timestamp after which `account` may transfer.

## Notes

Acquisition presence is tracked with an `Option` (not a `0` sentinel) so a balance acquired at ledger time `0` is handled correctly.
