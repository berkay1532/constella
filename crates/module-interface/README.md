# module-interface

The shared **ABI** between the compliance dispatcher and pluggable modules, plus the identity-provider boundary. Ships **no deployable contract** — only the contract types and generated clients (`ModuleClient`, `TokenClient`, `IdentityClient`) that components use to call one another. Mirrors OpenZeppelin's `ComplianceHook` surface so modules stay portable.

## What it defines

- **`ComplianceHook`** = `{ CanTransfer, CanCreate, Transferred, Created, Destroyed }` — the points a module can register against.
- **`ComplianceError`** — shared typed error space (e.g. `Denied = 6`).
- **`ComplianceModule`** trait → `ModuleClient`. Hooks: `can_transfer`, `can_create` (pre-checks, return `bool`); `transferred`, `created`, `destroyed` (post-events). A module implements only the hooks it needs.
- **`TokenRead`** trait → `TokenClient`. Minimal SEP-41 read surface (`balance`).
- **`IdentityProvider`** trait → `IdentityClient`. The attestor boundary: `country_of(account) -> Option<u32>`, `is_verified(account) -> bool`. Modules depend on this interface, never on the implementation (mock or ZK).

See [`CONTRIBUTING.md`](../../CONTRIBUTING.md) for how to build a module against this ABI.
