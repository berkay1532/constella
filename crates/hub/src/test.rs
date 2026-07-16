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
mod maxbal_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/constella_hub_module_max_balance.wasm"
    );
}
mod country_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/constella_hub_module_country_restrict.wasm"
    );
}
mod identity_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/constella_identity_mock.wasm"
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
        max_balance: 0,
        country_restrict: soroban_sdk::vec![&env],
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
            max_balance: 0,
            country_restrict: soroban_sdk::vec![&env],
        })
        .token;
    let tb = hub
        .launch(&LaunchConfig {
            admin: issuer_b.clone(),
            denylist: true,
            max_balance: 0,
            country_restrict: soroban_sdk::vec![&env],
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
            max_balance: 0,
            country_restrict: soroban_sdk::vec![&env],
        })
        .token;
    let bob = Address::generate(&env);

    env.set_auths(&[]); // no more mocked authorizations -> issuer.require_auth() must reject
    hub.add_to_denylist(&ta, &bob);
}

#[test]
fn two_tokens_max_balance_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let maxbal = env.register(maxbal_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "max_balance"), &maxbal);

    // token A cap 1000, token B cap 100
    let ta = hub
        .launch(&LaunchConfig {
            admin: Address::generate(&env),
            denylist: false,
            max_balance: 1000,
            country_restrict: soroban_sdk::vec![&env],
        })
        .token;
    let tb = hub
        .launch(&LaunchConfig {
            admin: Address::generate(&env),
            denylist: false,
            max_balance: 100,
            country_restrict: soroban_sdk::vec![&env],
        })
        .token;
    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    let alice = Address::generate(&env);

    tok_a.mint(&alice, &900); // under A's 1000 cap -> ok, mirror updates via hub fan-out
    assert_eq!(tok_a.balance(&alice), 900);
    assert!(tok_a.try_mint(&alice, &200).is_err()); // 900 + 200 > 1000 -> denied on A
                                                    // token B has cap 100 and alice's B-balance is independent (0)
    assert!(tok_b.try_mint(&alice, &200).is_err()); // > B cap 100
    tok_b.mint(&alice, &50); // under B's 100 cap -> ok
    assert_eq!(tok_b.balance(&alice), 50);
    assert_eq!(hub.max_balance(&ta), 1000);
    assert_eq!(hub.max_balance(&tb), 100);
}

// Same negative-auth pattern as `only_token_admin_can_denylist`: the forwarder's only auth
// gate is `Admin(token).require_auth()`. Launch a real token under `mock_all_auths`, drop the
// mocked authorizations, then confirm the forwarder call is rejected.
#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn only_token_admin_can_set_max_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let maxbal = env.register(maxbal_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "max_balance"), &maxbal);
    let t = hub
        .launch(&LaunchConfig {
            admin: Address::generate(&env),
            denylist: false,
            max_balance: 100,
            country_restrict: soroban_sdk::vec![&env],
        })
        .token;
    env.set_auths(&[]); // drop mocked auths -> the token's issuer did not authorize
    hub.set_max_balance(&t, &999);
}

// Each token that opts into country_restrict gets its OWN identity instance deployed at
// launch (admin = that token's issuer). The same real-world person can be attested
// differently on each token's identity — attesting US on A's identity and TR on B's must
// keep A's and B's eligibility fully isolated even though both tokens share one
// CountryRestrict module instance.
#[test]
fn two_tokens_country_restrict_isolated_end_to_end() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let identity_hash: BytesN<32> = env.deployer().upload_contract_wasm(identity_wasm::WASM);
    hub.set_identity_wasm(&identity_hash);
    let country = env.register(country_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "country_restrict"), &country);

    // token A allows US(840); token B allows DE(276)
    let ta = hub
        .launch(&LaunchConfig {
            admin: Address::generate(&env),
            denylist: false,
            max_balance: 0,
            country_restrict: soroban_sdk::vec![&env, 840u32],
        })
        .token;
    let tb = hub
        .launch(&LaunchConfig {
            admin: Address::generate(&env),
            denylist: false,
            max_balance: 0,
            country_restrict: soroban_sdk::vec![&env, 276u32],
        })
        .token;

    // Each token got its own identity; attest alice as US on A's, TR on B's.
    let id_a = identity_wasm::Client::new(&env, &hub.identity(&ta));
    let id_b = identity_wasm::Client::new(&env, &hub.identity(&tb));
    let alice = Address::generate(&env);
    id_a.set_country(&alice, &840);
    id_b.set_country(&alice, &792);

    let tok_a = token_wasm::Client::new(&env, &ta);
    let tok_b = token_wasm::Client::new(&env, &tb);
    tok_a.mint(&alice, &100); // US ∈ {US} on A -> ok
    assert_eq!(tok_a.balance(&alice), 100);
    assert!(tok_b.try_mint(&alice, &100).is_err()); // TR ∉ {DE} on B -> denied (isolated)
}

// Same negative-auth pattern as `only_token_admin_can_denylist`/`only_token_admin_can_set_max_balance`:
// the forwarder's only auth gate is `Admin(token).require_auth()`.
#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn only_token_admin_can_set_country_allow() {
    let env = Env::default();
    env.mock_all_auths();
    let (hub, hub_addr) = deploy_hub(&env);
    let token_hash: BytesN<32> = env.deployer().upload_contract_wasm(token_wasm::WASM);
    hub.set_token_wasm(&token_hash);
    let identity_hash: BytesN<32> = env.deployer().upload_contract_wasm(identity_wasm::WASM);
    hub.set_identity_wasm(&identity_hash);
    let country = env.register(country_wasm::WASM, (hub_addr.clone(),));
    hub.set_module_addr(&Symbol::new(&env, "country_restrict"), &country);
    let t = hub
        .launch(&LaunchConfig {
            admin: Address::generate(&env),
            denylist: false,
            max_balance: 0,
            country_restrict: soroban_sdk::vec![&env, 840u32],
        })
        .token;
    env.set_auths(&[]);
    hub.set_country_allow(&t, &soroban_sdk::vec![&env, 276u32]);
}
