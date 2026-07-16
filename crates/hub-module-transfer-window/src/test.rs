#![cfg(test)]
use crate::{TransferWindowHubModule, TransferWindowHubModuleClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

fn setup(env: &Env) -> (TransferWindowHubModuleClient<'static>, Address) {
    let hub = Address::generate(env);
    let id = env.register(TransferWindowHubModule, (hub.clone(),));
    (TransferWindowHubModuleClient::new(env, &id), hub)
}

#[test]
fn pause_and_window_isolated_per_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (m, _hub) = setup(&env);
    let ta = Address::generate(&env);
    let tb = Address::generate(&env);
    let x = Address::generate(&env);
    // both start open
    assert!(m.can_create(&x, &1, &ta) && m.can_create(&x, &1, &tb));
    m.pause(&ta);
    assert!(!m.can_create(&x, &1, &ta)); // A frozen
    assert!(m.can_create(&x, &1, &tb));  // B unaffected — isolated
    m.unpause(&ta);
    assert!(m.can_create(&x, &1, &ta));
    // window on A only
    m.set_window(&ta, &Some(100), &None);
    env.ledger().set_timestamp(50);
    assert!(!m.can_transfer(&x, &x, &1, &ta)); // before open_from on A
    assert!(m.can_transfer(&x, &x, &1, &tb));  // B has no window
    assert!(!m.is_paused(&ta));
    assert_eq!(m.window(&ta), (Some(100), None));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn pause_requires_hub_auth() {
    let env = Env::default();
    let (m, _hub) = setup(&env);
    m.pause(&Address::generate(&env));
}
