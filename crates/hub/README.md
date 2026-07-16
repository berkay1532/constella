# hub

The multi-tenant **compliance hub**. One deployed hub instance serves every token launched
through it: it is the `compliance` address each token points at, the dispatcher that fans out
the token's hook calls to whichever shared modules that token opted into, and the single
one-signature entry point (`launch`) that deploys a new token wired up correctly from the start.

## Multi-tenant model

Traditional compliance dispatchers (e.g. `crates/compliance`) are deployed **one per token** —
each token gets its own dispatcher instance holding its own module registrations. The hub
inverts that: **one hub, many tokens.** All per-token state is keyed by `token` in hub storage:

- `TokenAdmin(token) -> Address` — the token's issuer (set once, at `launch`).
- `Modules(token, hook) -> Vec<Address>` — which shared module addresses run on that token
  for that hook.

Compliance **modules themselves are also shared** — one deployed instance of, say,
`hub-module-denylist` serves every token, keying its own storage by `(token, account)` (see
`crates/hub-module-denylist`). So onboarding a new token costs zero new module deployments:
`launch` just adds registry entries pointing at the module addresses the platform admin already
configured on the hub.

This is what makes the two-token isolation property hold: two different tokens sharing the
*same* denylist module instance still see fully independent denylists, because every read/write
on the module is scoped by `token`. Denylisting `bob` on token A never touches token B.

## Platform-admin configuration

Before any token can launch, the platform admin (set at hub construction) configures the hub:

- `set_token_wasm(hash)` — the token Wasm to deploy for every `launch`.
- `set_module_addr(kind, addr)` — register a shared module's address under a `kind` symbol
  (e.g. `"denylist"`), so `launch` can wire it up by name.

Both require the platform admin's auth.

## `launch`: one-signature token deployment

```
launch(config: LaunchConfig { admin, denylist }) -> LaunchResult { token }
```

Requires only `config.admin.require_auth()` — the new issuer's signature. It:

1. Deploys the token Wasm with `(admin, hub_address)` as constructor args — the hub itself
   becomes the token's `compliance` address.
2. Records `TokenAdmin(token) = admin`.
3. If `denylist` is requested, registers the shared denylist module against the token's
   `CanCreate` and `CanTransfer` hooks.

## Hook surface (called by the token)

The token treats the hub exactly as it would treat any compliance dispatcher — it never knows
it's talking to a multi-tenant hub. On every mint/transfer the token passes **its own address**
as the trailing `token` argument, which is how the hub knows whose module registrations to
fan out to:

- `can_create(to, amount, token) -> bool` / `can_transfer(from, to, amount, token) -> bool` —
  pre-checks. AND-combines every module registered under `Modules(token, "CanCreate"/"CanTransfer")`
  via `ModuleClient`; short-circuits `false` on the first module that denies.
- `created(to, amount, token)` / `transferred(from, to, amount, token)` /
  `destroyed(from, amount, token)` — post-events. Each calls `token.require_auth()` first (the
  Phase-0 pattern: only the token contract itself, having just settled the mutation, can report
  it happened), then fans out to the modules registered under the matching hook.

### Hook-name constants

Hook names (`"CanCreate"`, `"CanTransfer"`, `"Transferred"`, `"Created"`, `"Destroyed"`) key the
`Modules(token, hook)` registry. Both the register side (`launch`) and the read side (the hook
fan-out above) go through a single `mod hooks { pub const CAN_CREATE: &str = ...; }` block in
`src/lib.rs` — never a raw string literal at either call site. This closes off a whole class of
"registered under a typo'd hook name, so it's silently never invoked" bugs.

## Issuer forwarders (denylist)

The hub also forwards issuer-gated writes to the shared denylist module, so an issuer never
needs to hold the module's address or auth directly:

- `add_to_denylist(token, account)` / `remove_from_denylist(token, account)` — require
  `TokenAdmin(token).require_auth()` (i.e. only *that* token's issuer), then call the shared
  denylist module via `DenylistClient`.
- `is_denied(token, account) -> bool` — unauthenticated read passthrough.

This is the hub's single auth surface for per-token module administration: the module itself
(`hub-module-denylist`) trusts only the hub's own `require_auth()` (see its README), and the hub
gates that trust per-token via `TokenAdmin`.

## Build-order note

Hub tests `contractimport!` the built token and denylist module Wasm (see `src/test.rs`), so
`stellar contract build` must run **before** `cargo test -p constella-hub` — a plain `cargo test`
against stale or missing Wasm artifacts will fail to compile the test module or exercise stale
contract code:

```bash
stellar contract build
cargo test -p constella-hub
```

## Dependencies

The hub depends only on `constella-module-interface` (`ModuleClient`, `DenylistClient`) for
cross-contract calls — never on a concrete module or token `#[contract]` crate. This keeps the
hub decoupled from every module's implementation; it only needs the shared ABI.
