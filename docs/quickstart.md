# Quickstart — deploy your own compliant token

This guide walks you from an empty machine to a **permissioned token on Stellar testnet** whose transfers are gated by a set of composable compliance modules — the exact stack Constella ships. You pick which rules apply; the token enforces them on every mint and transfer.

> All commands target **testnet**, where accounts are funded for free by Friendbot. Nothing here costs real money.

## 1. Prerequisites

- [Rust](https://rustup.rs/) with the `wasm32v1-none` target: `rustup target add wasm32v1-none`
- The [`stellar` CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (v22+): `cargo install --locked stellar-cli`
- This repository, built: `stellar contract build`

## 2. The mental model

Every regulated transfer passes through two layers:

- **Identity** — "is this address eligible?" (an attestor answers, e.g. `country_of`, `is_verified`).
- **Compliance** — "does this transfer satisfy the rules?" (the module library).

A **dispatcher** contract (`crates/compliance`) holds a per-hook registry of **modules**, AND-combines their pre-checks (`can_transfer` / `can_create`), and fans out post-events (`transferred` / `created` / `destroyed`) to the stateful ones. Your token calls the dispatcher on every mint/transfer and reverts if any module says no.

```
token.mint/transfer ──▶ dispatcher.can_* (AND of all modules) ──▶ revert if any false
                                     └─▶ dispatcher.<post-event> ──▶ stateful modules update
```

## 3. Pick your modules

| Crate | Rule | Identity? | Stateful? | Hooks |
|---|---|---|---|---|
| `module-max-holders` | cap total number of holders | no | yes | all 5 |
| `module-max-balance` | cap per-holder balance | no | yes | all 5 |
| `module-lockup` | tokens locked for N seconds after acquisition | no | yes | transfer + post |
| `module-country-restrict` | only allow-listed countries may hold | **yes** | no | `can_*` |
| `module-denylist` | sanctioned addresses may not send/receive | no | yes (own set) | `can_*` |
| `module-max-investors-per-country` | cap distinct holders per country | **yes** | yes | all 5 |
| `module-transfer-window` | freeze + time-window on all movement | no | config only | `can_*` |

> **Registration rule:** a module that keeps its own balance mirror (`max-holders`, `max-balance`, `max-investors-per-country`) **must** be registered on the post-event hooks (`Created`, `Transferred`, `Destroyed`) **from genesis**, or its mirror will drift. See [`CONTRIBUTING.md`](../CONTRIBUTING.md).

## 4. Deploy — the guided path

The fastest way to see it end-to-end is the bundled script, which deploys the full stack, wires the modules, sets identities, and runs real pass/revert transactions:

```bash
bash scripts/deploy-testnet.sh
```

It writes every deployed address to [`scripts/deployed.testnet.json`](../scripts/deployed.testnet.json) and prints a `stellar.expert` explorer link. Read the script top-to-bottom — it is the canonical, copy-pasteable reference for the manual steps below.

## 5. Deploy — the manual path

Set up an admin identity and a couple of holders:

```bash
NET=testnet
WASM=target/wasm32v1-none/release
stellar keys generate admin --network $NET --fund
stellar keys generate alice --network $NET --fund
ADMIN=$(stellar keys address admin); ALICE=$(stellar keys address alice)
```

Deploy the attestor, the dispatcher, and your chosen modules (constructor args follow `--`):

```bash
# identity attestor (mock; swap for a real KYC provider in production)
IDENTITY=$(stellar contract deploy --wasm $WASM/constella_identity_mock.wasm \
  --source admin --network $NET -- --admin $ADMIN)

# the compliance dispatcher
COMPLIANCE=$(stellar contract deploy --wasm $WASM/constella_compliance.wasm \
  --source admin --network $NET -- --admin $ADMIN)

# example modules
DENYLIST=$(stellar contract deploy --wasm $WASM/constella_module_denylist.wasm \
  --source admin --network $NET -- --admin $ADMIN)
WINDOW=$(stellar contract deploy --wasm $WASM/constella_module_transfer_window.wasm \
  --source admin --network $NET -- --admin $ADMIN)
INVESTORS=$(stellar contract deploy --wasm $WASM/constella_module_max_investors_per_country.wasm \
  --source admin --network $NET -- --admin $ADMIN --identity $IDENTITY --cap 100)
```

Deploy your token, pointed at the dispatcher:

```bash
TOKEN=$(stellar contract deploy --wasm $WASM/constella_demo_token.wasm \
  --source admin --network $NET -- --admin $ADMIN --compliance $COMPLIANCE)
```

Register modules on their hooks (pure pre-checks vs. mirror modules):

```bash
reg() { stellar contract invoke --id $COMPLIANCE --source admin --network $NET -- \
  add_module_to --hook "$1" --module "$2"; }

for h in CanCreate CanTransfer; do reg $h $DENYLIST; reg $h $WINDOW; done
for h in CanCreate CanTransfer Created Transferred Destroyed; do reg $h $INVESTORS; done
```

Attest an identity and mint — the token now enforces every registered rule:

```bash
stellar contract invoke --id $IDENTITY --source admin --network $NET -- \
  set_country --account $ALICE --code 840   # US
stellar contract invoke --id $TOKEN --source admin --network $NET -- \
  mint --to $ALICE --amount 500
```

## 6. Prove the rules bite

```bash
# Freeze the token → any mint/transfer reverts
stellar contract invoke --id $WINDOW --source admin --network $NET -- pause
stellar contract invoke --id $TOKEN  --source admin --network $NET -- mint --to $ALICE --amount 1
#   ↳ reverts with a compliance error
stellar contract invoke --id $WINDOW --source admin --network $NET -- unpause

# Sanction an address → transfers to it revert
stellar contract invoke --id $DENYLIST --source admin --network $NET -- add_to_denylist --account $ALICE
stellar contract invoke --id $DENYLIST --source admin --network $NET -- is_denied --account $ALICE   # true
```

## 7. Going further

- **Write your own module:** [`CONTRIBUTING.md`](../CONTRIBUTING.md) has the ABI and the no-re-entrancy rule.
- **Production:** replace `identity-mock` with a real attestor (or the ZK-backed provider in `crates/module-identity-zk`), and this same dispatcher/token pattern is what the OpenZeppelin RWA token uses.
- **Mainnet:** the flow is identical with `--network mainnet` and a funded source account — review the deploy checklist first.
