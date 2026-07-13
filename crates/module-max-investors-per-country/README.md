# module-max-investors-per-country

**Rule:** no single country may exceed a maximum number of *distinct holders* (investor-count limit, e.g. Reg-A / Reg-S style caps).

| | |
|---|---|
| Hooks | `can_transfer`, `can_create`, `transferred`, `created`, `destroyed` |
| Stateful | **Yes** — self-tracked balance mirror + per-country holder counts |
| Identity-dependent | **Yes** — reads `country_of(account)` from the identity provider |

## Behaviour

Each holder is bucketed into a country via the configured `IdentityProvider`. The module maintains its **own balance mirror** (from the post-event hooks) so it can detect holder transitions — `0 → positive` = *joins*, `positive → 0` = *leaves* — and keep a per-country holder count without re-entering the token.

Pre-checks:
- **`can_create`** denies a mint whose recipient would become a *new* holder in a country already at the cap. Existing holders (topping up) consume no new slot.
- **`can_transfer`** denies when `to` would be a new holder in a full country — **unless** `from` fully exits that same country, in which case the freed slot offsets it (net zero) and the transfer is allowed.

A recipient with no attested country cannot be bucketed and is **denied** (conservative: an unattributable holder can't be counted against any cap).

## Config

Constructor: `--admin <Address> --identity <Address> --cap <u32>` (a single per-country cap applied to every country).

## Admin operations

- `set_cap(cap)` — update the per-country cap. Admin-only.

## Read accessors

- `cap()` — the per-country holder cap.
- `count(country)` — current distinct holders attributed to `country`.

## Notes

⚠️ Must be registered on the **post-event hooks from genesis** so the mirror and per-country counts stay accurate. See the re-entrancy note in [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
