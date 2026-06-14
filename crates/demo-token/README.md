# demo-token

A minimal **SEP-41-style permissioned token** used to exercise the compliance engine. Every mutating call routes through the dispatcher.

## Flow

- **`mint(to, amount)`** → `compliance.can_create` (deny ⇒ revert `Denied`) → update balance/supply → `compliance.created`.
- **`transfer(from, to, amount)`** → `from.require_auth()` → `compliance.can_transfer` (deny ⇒ revert) → move balance → `compliance.transferred`.
- Reads: `balance(id)`, `total_supply()`, `compliance()`.

## Config

Constructor: `--admin <Address> --compliance <Address>`.

## Notes

This is a **reference/demo** token — not a production SEP-41 implementation. It exists to show the dispatcher path end-to-end (it also carries the workspace integration test for transfer/mint pass + revert).
