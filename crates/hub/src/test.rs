#![cfg(test)]
use crate::{Hub, HubClient, LaunchConfig};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol};

mod token_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/constella_demo_token.wasm"
    );
}
mod denylist_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/constella_hub_module_denylist.wasm"
    );
}

fn deploy_hub(env: &Env) -> (HubClient<'static>, Address) {
    let admin = Address::generate(env);
    let id = env.register(Hub, (admin.clone(),));
    (HubClient::new(env, &id), id)
}

#[test]
fn launch_deploys_token_and_wires_denylist() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);

    // platform admin config: token wasm + the shared denylist module address.
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let denylist = env.register(denylist_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "denylist"), &denylist);

    let issuer = Address::generate(&env);
    let res = hub.launch(&LaunchConfig {
        admin: issuer.clone(),
        denylist: true,
    });

    assert_eq!(hub.token_admin(&res.token), issuer);
    // denylist is registered for this token on both pre-check hooks
    let on_create = hub.modules_for(&res.token, &Symbol::new(&env, "CanCreate"));
    assert!(on_create.contains(&denylist));
}

#[test]
fn two_tokens_denylist_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let denylist = env.register(denylist_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "denylist"), &denylist);

    let issuer_a = Address::generate(&env);
    let issuer_b = Address::generate(&env);
    let ta = hub
        .launch(&LaunchConfig {
            admin: issuer_a.clone(),
            denylist: true,
        })
        .token;
    let tb = hub
        .launch(&LaunchConfig {
            admin: issuer_b.clone(),
            denylist: true,
        })
        .token;

    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    tok_a.mint(&alice, &100);
    tok_b.mint(&alice, &100);
    // issuer A denylists bob on token A only
    hub.add_to_denylist(&ta, &bob);
    assert!(tok_a.try_transfer(&alice, &bob, &10).is_err()); // blocked on A
    tok_b.transfer(&alice, &bob, &10); // allowed on B
    assert_eq!(tok_b.balance(&bob), 10);
}

// The forwarder's only auth gate is `Admin(token).require_auth()` (the token's own issuer).
// Set up a real launched token under `mock_all_auths`, then drop the mocked authorizations
// before calling the forwarder: with no authorization entries present, `require_auth()` for
// the issuer must reject the call.
#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn only_token_admin_can_denylist() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let denylist = env.register(denylist_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "denylist"), &denylist);

    let issuer = Address::generate(&env);
    let ta = hub
        .launch(&LaunchConfig {
            admin: issuer,
            denylist: true,
        })
        .token;
    let bob = Address::generate(&env);

    env.set_auths(&[]); // no more mocked authorizations -> issuer.require_auth() must reject
    hub.add_to_denylist(&ta, &bob);
}
