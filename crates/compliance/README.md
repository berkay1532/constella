# compliance

The **dispatcher** (compliance engine). Holds a per-hook registry of module addresses and runs them on behalf of a token.

## Behaviour

- Stores `Map<ComplianceHook, Vec<Address>>` of registered modules.
- **`can_transfer` / `can_create`** — call every registered module's pre-check and **AND-combine**: any `false` ⇒ deny.
- **`transferred` / `created` / `destroyed`** — **fan out** to every registered module so stateful ones update their bookkeeping.
- **`add_module_to(hook, module)` / `remove_module_from(hook, module)`** — admin-only registry management.
- **`get_modules_for_hook(hook)`** — read the registered modules for a hook.

## Config

Constructor: `--admin <Address>`. The admin manages the registry.

## Notes

A token (see `demo-token`) calls `can_*` before mutating state and `transferred`/`created`/`destroyed` after. Modules that maintain a balance mirror must be registered on the post-event hooks from genesis. See [`docs/architecture.md`](../../docs/architecture.md).
