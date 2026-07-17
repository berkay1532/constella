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
- `set_identity_wasm(hash)` — the identity-provider Wasm to deploy, one fresh instance per
  token, whenever a launch opts into `country_restrict`.
- `set_module_addr(kind, addr)` — register a shared module's address under a `kind` symbol
  (e.g. `"denylist"`, `"country_restrict"`), so `launch` can wire it up by name.

All three require the platform admin's auth.

## `launch`: one-signature token deployment

```
launch(config: LaunchConfig { admin, denylist, max_balance, country_restrict, max_holders, lockup, transfer_window, max_investors }) -> LaunchResult { token }
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
5. If `country_restrict` is non-empty (empty means "not selected"), deploys a **brand-new
   identity-provider instance for this token** — `(admin,)` as its constructor arg, so the
   token's own issuer is that identity's attestor — records `Identity(token) = identity`,
   registers the shared `country_restrict` module against `CanCreate` and `CanTransfer`, then
   calls `CountryRestrictClient::configure(token, identity, country_restrict)` to point the
   shared module at this token's fresh identity and allow-list in one call.
6. If `max_holders > 0` (0 means "not selected"), registers the shared MaxHolders module
   against **all 5 hooks** (same reasoning as MaxBalance: it needs the post-events to keep its
   per-token holder count in sync), then calls `MaxHoldersClient::set_max(token, max_holders)`
   to initialize that token's holder cap.
7. If `lockup > 0` (0 means "not selected"), registers the shared Lockup module against
   `CanTransfer`, `Created`, and `Transferred` (it only needs to record acquisition time on
   mint/transfer-in and check it on transfer-out — no `CanCreate`/`Destroyed` gating), then
   calls `LockupClient::set_duration(token, lockup)` to initialize that token's lock duration.
8. If `transfer_window` is `true`, registers the shared TransferWindow module against
   `CanCreate` and `CanTransfer` only — it is a pure pre-check gate (pause/window state), so it
   needs no post-event hooks. No config call is made at launch: a fresh registration starts
   unpaused with an all-time-open window; the issuer opts into pausing/windowing later via the
   forwarders below.
9. If `max_investors > 0` (0 means "not selected"), registers the shared MaxInvestorsPerCountry
   module against **all 5 hooks** (it needs the post-events to keep its per-holder balance mirror
   and per-country holder count in sync), then calls
   `MaxInvestorsClient::configure(token, identity, max_investors)` to point the shared module at
   this token's identity and per-country cap. Like `country_restrict`, it is identity-dependent —
   it reads each holder's country to bucket the count. **When a token selects both
   `country_restrict` and `max_investors`, the two share the single per-token identity instance:**
   `launch` deploys the identity **once** if either identity-dependent module is selected (see the
   hoisted deploy in `src/lib.rs`), then both modules are configured against that same
   `Identity(token)`. A token selecting only `max_investors` still gets its own dedicated identity
   instance, same as the country-restrict-only case.

## Per-token identity model

Unlike denylist/MaxBalance (whose *module* is shared but whose *state* is merely keyed by
token), the **identity-dependent** modules — `country_restrict` and `max_investors` — get a
**dedicated identity instance per token**, deployed at `launch` time. This is deliberate: an
account's attested country is a claim made *about that person, for that token's compliance
context* — token A's issuer attesting "this account is US-resident" should never leak into or be
confused with token B's issuer's own attestation of the same real-world person. A token that
selects both identity-dependent modules gets **one** shared identity instance (the deploy is
hoisted in `launch`), so `country_restrict` and `max_investors` read the same attestations for
that token. Two tokens sharing one `country_restrict`/`max_investors` module instance still get
fully independent identity data because:

- Each token's identity is its own deployed contract instance (own storage, own admin — that
  token's issuer).
- The shared `country_restrict` module stores `Identity(token) -> that instance's address` and
  reads through it (`IdentityClient::country_of`) only for that token's checks.

The issuer attests directly on `identity(token)` (see below) — there is no hub forwarder for
attestation itself, only for the allow-list.

- `identity(token) -> Address` — unauthenticated read; returns that token's identity instance
  address so the issuer (or any integrator) can call `set_country`/`set_verified` on it
  directly, `require_auth`-gated by that identity instance's own admin (the issuer).

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

## Issuer forwarders (denylist, MaxBalance, CountryRestrict, MaxHolders, Lockup, TransferWindow)

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
- `set_country_allow(token, codes)` — requires `TokenAdmin(token).require_auth()`, then calls
  the shared country-restrict module via `CountryRestrictClient::set_allowed` to change that
  token's allow-list after launch. (Attestation itself is not forwarded — see "Per-token
  identity model" above; the issuer calls the identity instance directly.)
- `set_max_holders(token, cap)` — requires `TokenAdmin(token).require_auth()`, then calls the
  shared MaxHolders module via `MaxHoldersClient::set_max` to change that token's holder cap
  after launch.
- `max_holders(token) -> u32` / `holders(token) -> u32` — unauthenticated read passthroughs
  (configured cap / current holder count).
- `set_lockup(token, secs)` — requires `TokenAdmin(token).require_auth()`, then calls the
  shared Lockup module via `LockupClient::set_duration` to change that token's lock duration
  after launch.
- `unlock_at(token, holder) -> u64` — unauthenticated read passthrough (ledger timestamp at
  which that holder's tokens unlock; `0` if never acquired).
- `pause(token)` / `unpause(token)` — require `TokenAdmin(token).require_auth()`, then call the
  shared TransferWindow module via `TransferWindowClient` to freeze/unfreeze that token
  immediately, independent of any configured window.
- `set_window(token, open_from, open_until)` — requires `TokenAdmin(token).require_auth()`,
  then calls `TransferWindowClient::set_window` to (re)configure that token's open interval.
- `is_paused(token) -> bool` / `transfer_window(token) -> (Option<u64>, Option<u64>)` —
  unauthenticated read passthroughs.
- `set_investor_cap(token, cap)` — requires `TokenAdmin(token).require_auth()`, then calls the
  shared MaxInvestorsPerCountry module via `MaxInvestorsClient::set_cap` to change that token's
  per-country holder cap after launch.
- `investor_cap(token) -> u32` / `investor_count(token, country) -> u32` — unauthenticated read
  passthroughs (configured per-country cap / current distinct-holder count for a country).

This is the hub's single auth surface for per-token module administration: each module itself
(`hub-module-denylist`, `hub-module-max-balance`, `hub-module-country-restrict`,
`hub-module-max-holders`, `hub-module-lockup`, `hub-module-transfer-window`,
`hub-module-max-investors-per-country`) trusts only the
hub's own `require_auth()` (see their READMEs), and the hub gates that trust per-token via
`TokenAdmin`. All forwarder families resolve their module address through the same private
`module_addr(env, kind)` helper — a single lookup shared by every module kind, rather than one
hand-written accessor per module.

## Build-order note

Hub tests `contractimport!` the built token, denylist, max-balance, country-restrict,
identity-mock, max-holders, lockup, transfer-window, and max-investors-per-country Wasm (see
`src/test.rs`), so
`stellar contract build` must run **before** `cargo test -p constella-hub` — a plain
`cargo test` against stale or missing Wasm artifacts will fail to compile the test module or
exercise stale contract code:

```bash
stellar contract build
cargo test -p constella-hub
```

## Dependencies

The hub depends only on `constella-module-interface` (`ModuleClient`, `DenylistClient`,
`MaxBalanceClient`, `CountryRestrictClient`, `MaxHoldersClient`, `LockupClient`,
`TransferWindowClient`, `MaxInvestorsClient`) for cross-contract calls — never on a concrete module or token
`#[contract]` crate. This keeps the hub decoupled from every module's implementation; it only
needs the shared ABI. The per-token identity instances the hub deploys (see "Per-token identity
model" above) are likewise reached only by `Address` (`deploy_v2` against a
platform-configured Wasm hash) — the hub never depends on the identity-mock `#[contract]` crate
either.

## Live testnet evidence — one-signature launch

Verified on Stellar testnet: the hub + a shared denylist module were deployed once, then a funded issuer launched a full compliant token in **ONE signed transaction**, and the shared module enforced the denylist live.

- **One-signature launch tx:** [`172f634c…`](https://stellar.expert/explorer/testnet/tx/172f634ce7bc9f26db010eeb767e7d2d31a78bc40362c8d38bfb59b49cbe7422) → launched token `CDZQI5NDI2U6QEQXSYXFRBDT5OTNNJBKMQEDB7Z5PU2ZSI4DWEJKKPRG`
- Mint to a holder passed; after the issuer denylisted an account (forwarded through the hub, `is_denied = true`), a transfer to it **reverted** — the shared denylist instance enforces per-token, live.
- Hub `CDSZ22AN…NETQ`, shared denylist `CA7LHK4K…QPWT`. Re-run with `scripts/` equivalents to regenerate.

## Live testnet evidence — per-token MaxBalance cap

The stateful balance-mirror path (updated through the hub's post-event fan-out) enforces a per-token cap live:
- **One-signature launch** (cap 1000): tx [`a08c07ba…`](https://stellar.expert/explorer/testnet/tx/a08c07ba3a4a3a12e5a94c9d9b7bc8914d21e93d4110baca7f44e6f610dd9226) → token `CDTRFHTR2EEXWV5MAG2W5ZXKCVQ5OH4VEDBJES7HZA4JZLAPHCDEH63X`.
- Mint 900 passed (under cap; the shared module's `Bal(token, holder)` mirror updated via the hub's `created` fan-out); a further mint of 200 **reverted** (900 + 200 > 1000) with the balance unchanged at 900 — the cap enforced live on a shared module instance.

## Live testnet evidence — per-token identity (CountryRestrict)

The per-token identity mechanic enforces a country allow-list live:
- **One-signature launch** (allow US=840): tx [`0778f025…`](https://stellar.expert/explorer/testnet/tx/0778f0252bb7ea88a472f6469fffd22717ab9582cf282ad30cdaef023af23cd6) → token `CDNKCDLQ6MJBS7AOTA6RWWJNCTSLF7KWTRDQ4UTDQQKCWG7BWSOXRBKU`.
- The hub deployed a **dedicated identity provider for that token**, read back via `hub.identity(token)` → `CA4QS7SNEAJZIMF22IXCPEXLPFRTHSP3ZGDQF74QOTLKVKRGIBGTWLFY`. The issuer (its admin) attested `alice = US(840)` and `carol = TR(792)` directly on it.
- Minting to alice passed (US ∈ {US}); minting to carol **reverted** (TR ∉ {US}) — CountryRestrict enforced live, reading each token's own identity.

## Live testnet evidence — MaxHolders + TransferWindow

Both enforce per token, live, from one launch:
- **One-signature launch** (`max_holders: 1, transfer_window: true`): tx [`849d7b3d…`](https://stellar.expert/explorer/testnet/tx/849d7b3dc1261b2824df61b4fa23f8afcc9feb05b14fd3d3f1cc85770daff701).
- Minting a 1st holder passed (`holders = 1`); minting a 2nd holder **reverted** (MaxHolders cap 1). Then `hub.pause(token)` (`is_paused = true`) made a mint **revert** (frozen), and `hub.unpause(token)` let it pass again — the shared modules enforce and the issuer's forwarders drive per-token config, live.

## Live testnet evidence — MaxInvestorsPerCountry + shared per-token identity (completes 7/7)

The last module combines the stateful mirror and the per-token identity, and the launch proves the **shared-identity** design: one token opted into BOTH `country_restrict` and `max_investors`, and a single identity instance served both.
- **One-signature launch** (`country_restrict: [840], max_investors: 1`): tx [`f0e11b8e…`](https://stellar.expert/explorer/testnet/tx/f0e11b8e3b122dd3731e1ecdfd7ff8a49b62f24555281ce964f74bd986247475) → token `CCDP3HURWJPLFYQ4OZPA6YY7UMNA5ZNZO3HKCPEB6HJVOOU6FGWCT4C5`, `investor_cap = 1`.
- The hub deployed **one** identity for the token (read back via `hub.identity(token)` → `CBKNFJBEVAWYA3RFNGHR6RL5ZC62L6ADOBP7QP7IYUSQGINTTRMDZKIH`); the issuer attested `alice = US(840)` and `bob = US(840)` on it. That the attestation drove the MaxInvestors count proves both modules read the **same** identity.
- Minting to alice passed (`investor_count(US) = 1`); minting to bob **reverted** (2nd US holder, per-country cap 1). Then `hub.set_investor_cap(token, 2)` bumped the cap and bob's mint **passed** (`investor_count(US) = 2`) — the shared MaxInvestors instance enforces per (token, country), live, and the issuer's forwarder drives the cap.

All 7 modules are now proven live on testnet from one-signature launches.
