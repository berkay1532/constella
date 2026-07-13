# module-transfer-window

**Rule:** the token may only move when it is not frozen **and** the ledger clock is inside an allowed time window.

| | |
|---|---|
| Hooks | `can_transfer`, `can_create` |
| Stateful | Config-only (no balance mirror) |
| Identity-dependent | No |

## Behaviour

Two orthogonal admin controls:

1. **Pause / freeze** — `pause()` halts *all* transfers and mints instantly (an emergency freeze); `unpause()` resumes. Pause always wins, even inside an open window.
2. **Time window** — an optional `[open_from, open_until]` range of ledger timestamps. Outside it, transfers and mints are denied. Both bounds are **inclusive** and either may be left `None` for an open-ended side (e.g. `open_from` only = a lockup that ends at a date; `open_until` only = a sale that closes).

The module reads only its own config and `env.ledger().timestamp()` — never a balance or an identity provider — so there is no re-entrancy concern and no post-event bookkeeping.

## Config

Constructor: `--admin <Address>`. Starts **unpaused with no window** (fully open).

## Admin operations

- `pause()` / `unpause()` — freeze / resume. Admin-only.
- `set_window(open_from, open_until)` — set the allowed window (`Option<u64>` each). Admin-only.

## Read accessors

- `is_paused()` — current freeze state.
- `window()` — `(open_from, open_until)`.

## Notes

Both `can_transfer` and `can_create` are gated, so a freeze is a *full* halt (issuer mints are stopped too). See [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
