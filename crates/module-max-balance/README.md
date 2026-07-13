# module-max-balance

**Rule:** no single holder may exceed a maximum balance (concentration cap).

| | |
|---|---|
| Hooks | `can_transfer`, `can_create`, `transferred`, `created`, `destroyed` |
| Stateful | **Yes** — self-tracked balance mirror |
| Identity-dependent | No |

## Behaviour

Pre-check denies a transfer/mint if `mirror[to] + amount > cap`. The module keeps its **own balance mirror** (updated from the post-event hooks) so it never re-enters the token to read a balance.

## Config

Constructor: `--admin <Address> --max_per_holder <i128>`.

## Read accessors

- `max()` — the per-holder cap.

## Notes

⚠️ Must be registered on the **post-event hooks from genesis** so the mirror stays accurate. See the re-entrancy note in [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
