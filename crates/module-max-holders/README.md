# module-max-holders

**Rule:** the token may have at most **N** distinct holders.

| | |
|---|---|
| Hooks | `can_transfer`, `can_create`, `transferred`, `created`, `destroyed` |
| Stateful | **Yes** — self-tracked balance mirror + holder count |
| Identity-dependent | No |

## Behaviour

Pre-check denies a transfer/mint that would introduce a **new** holder once the holder count is at the cap. To know whether `to` is new without re-entering the token, the module keeps its **own balance mirror**, updated from the post-event hooks (`created`/`transferred`/`destroyed`).

## Config

Constructor: `--admin <Address> --max <u32>`.

## Read accessors

- `holders()` — current distinct-holder count.
- `max()` — the cap.

## Notes

⚠️ Must be registered on the **post-event hooks from genesis**, or its mirror drifts. See the re-entrancy note in [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
