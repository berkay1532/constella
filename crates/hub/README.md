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
launch(config: LaunchConfig { admin, denylist, max_balance }) -> LaunchResult { token }
```

Requires only `config.admin.require_auth()` — the new issuer's signature. It:

1. Deploys the token Wasm with `(admin, hub_address)` as constructor args — the hub itself
   becomes the token's `compliance` address.
2. Records `TokenAdmin(token) = admin`.
3. If `denylist` is requested, registers the shared denylist module against the token's
   `CanCreate` and `CanTransfer` hooks.
4. If `max_balance > 0` (0 means "not selected"), registers the shared max-balance module
   against **all 5 hooks** (`CanCreate`, `CanTransfer`, `Created`, `Transferred`, `Destroyed` —
   it needs the post-events to keep its per-holder balance mirror in sync, unlike the
   stateless denylist), then calls `MaxBalanceClient::set_max(token, max_balance)` to
   initialize that token's cap on the shared module.

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

## Issuer forwarders (denylist, MaxBalance)

The hub also forwards issuer-gated writes to the shared modules, so an issuer never needs to
hold a module's address or auth directly:

- `add_to_denylist(token, account)` / `remove_from_denylist(token, account)` — require
  `TokenAdmin(token).require_auth()` (i.e. only *that* token's issuer), then call the shared
  denylist module via `DenylistClient`.
- `is_denied(token, account) -> bool` — unauthenticated read passthrough.
- `set_max_balance(token, cap)` — requires `TokenAdmin(token).require_auth()`, then calls the
  shared max-balance module via `MaxBalanceClient::set_max` to change that token's cap after
  launch.
- `max_balance(token) -> i128` — unauthenticated read passthrough.

This is the hub's single auth surface for per-token module administration: each module itself
(`hub-module-denylist`, `hub-module-max-balance`) trusts only the hub's own `require_auth()`
(see their READMEs), and the hub gates that trust per-token via `TokenAdmin`. Both forwarder
families resolve their module address through the same private `module_addr(env, kind)`
helper — a single lookup shared by every module kind, rather than one hand-written accessor
per module.

## Build-order note

Hub tests `contractimport!` the built token, denylist, and max-balance module Wasm (see
`src/test.rs`), so `stellar contract build` must run **before** `cargo test -p constella-hub` —
a plain `cargo test` against stale or missing Wasm artifacts will fail to compile the test
module or exercise stale contract code:

```bash
stellar contract build
cargo test -p constella-hub
```

## Dependencies

The hub depends only on `constella-module-interface` (`ModuleClient`, `DenylistClient`,
`MaxBalanceClient`) for cross-contract calls — never on a concrete module or token `#[contract]`
crate. This keeps the hub decoupled from every module's implementation; it only needs the
shared ABI.

## Live testnet evidence — one-signature launch

Verified on Stellar testnet: the hub + a shared denylist module were deployed once, then a funded issuer launched a full compliant token in **ONE signed transaction**, and the shared module enforced the denylist live.

- **One-signature launch tx:** [`172f634c…`](https://stellar.expert/explorer/testnet/tx/172f634ce7bc9f26db010eeb767e7d2d31a78bc40362c8d38bfb59b49cbe7422) → launched token `CDZQI5NDI2U6QEQXSYXFRBDT5OTNNJBKMQEDB7Z5PU2ZSI4DWEJKKPRG`
- Mint to a holder passed; after the issuer denylisted an account (forwarded through the hub, `is_denied = true`), a transfer to it **reverted** — the shared denylist instance enforces per-token, live.
- Hub `CDSZ22AN…NETQ`, shared denylist `CA7LHK4K…QPWT`. Re-run with `scripts/` equivalents to regenerate.

## Live testnet evidence — per-token MaxBalance cap

The stateful balance-mirror path (updated through the hub's post-event fan-out) enforces a per-token cap live:
- **One-signature launch** (cap 1000): tx [`a08c07ba…`](https://stellar.expert/explorer/testnet/tx/a08c07ba3a4a3a12e5a94c9d9b7bc8914d21e93d4110baca7f44e6f610dd9226) → token `CDTRFHTR2EEXWV5MAG2W5ZXKCVQ5OH4VEDBJES7HZA4JZLAPHCDEH63X`.
- Mint 900 passed (under cap; the shared module's `Bal(token, holder)` mirror updated via the hub's `created` fan-out); a further mint of 200 **reverted** (900 + 200 > 1000) with the balance unchanged at 900 — the cap enforced live on a shared module instance.
