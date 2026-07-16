#![cfg(test)]
use crate::{Hub, HubClient, LaunchConfig};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol};

mod token_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_demo_token.wasm"); }
mod denylist_wasm { soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/constella_hub_module_denylist.wasm"); }

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
    let res = hub.launch(&LaunchConfig { admin: issuer.clone(), denylist: true });

    assert_eq!(hub.token_admin(&res.token), issuer);
    // denylist is registered for this token on both pre-check hooks
    let on_create = hub.modules_for(&res.token, &Symbol::new(&env, "CanCreate"));
    assert!(on_create.contains(&denylist));
}
