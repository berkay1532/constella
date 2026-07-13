#![cfg(test)]
//! Unit tests for the TransferWindow (freeze + time-window) compliance module.

use crate::{TransferWindowModule, TransferWindowModuleClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env};

struct Fixture<'a> {
    env: Env,
    module: TransferWindowModuleClient<'a>,
    from: Address,
    to: Address,
    token: Address,
}

fn setup() -> Fixture<'static> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let m = env.register(TransferWindowModule, (admin,));
    let module = TransferWindowModuleClient::new(&env, &m);
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let token = Address::generate(&env);
    Fixture {
        env,
        module,
        from,
        to,
        token,
    }
}

impl Fixture<'_> {
    fn can_transfer(&self) -> bool {
        self.module
            .can_transfer(&self.from, &self.to, &100, &self.token)
    }
    fn can_create(&self) -> bool {
        self.module.can_create(&self.to, &100, &self.token)
    }
}

#[test]
fn starts_open() {
    let f = setup();
    assert!(!f.module.is_paused());
    assert!(f.can_transfer());
    assert!(f.can_create());
}

#[test]
fn pause_blocks_transfers_and_mints() {
    let f = setup();
    f.module.pause();
    assert!(f.module.is_paused());
    assert!(!f.can_transfer(), "paused → transfer denied");
    assert!(!f.can_create(), "paused → mint denied");
}

#[test]
fn unpause_reopens() {
    let f = setup();
    f.module.pause();
    f.module.unpause();
    assert!(!f.module.is_paused());
    assert!(f.can_transfer());
}

#[test]
#[should_panic]
fn pause_requires_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let m = env.register(TransferWindowModule, (admin,));
    let module = TransferWindowModuleClient::new(&env, &m);
    module.pause(); // no admin auth provided → must panic
}

#[test]
fn window_accessor_returns_configured() {
    let f = setup();
    f.module.set_window(&Some(100), &Some(200));
    assert_eq!(f.module.window(), (Some(100), Some(200)));
}

#[test]
fn window_before_open_denies() {
    let f = setup();
    f.module.set_window(&Some(100), &None);
    f.env.ledger().set_timestamp(50);
    assert!(!f.can_transfer(), "before open_from → denied");
    assert!(!f.can_create(), "before open_from → denied");
}

#[test]
fn window_within_range_allows() {
    let f = setup();
    f.module.set_window(&Some(100), &Some(200));
    f.env.ledger().set_timestamp(150);
    assert!(f.can_transfer());
    assert!(f.can_create());
}

#[test]
fn window_after_close_denies() {
    let f = setup();
    f.module.set_window(&None, &Some(200));
    f.env.ledger().set_timestamp(300);
    assert!(!f.can_transfer(), "after open_until → denied");
}

#[test]
fn window_boundaries_are_inclusive() {
    let f = setup();
    f.module.set_window(&Some(100), &Some(200));
    f.env.ledger().set_timestamp(100);
    assert!(f.can_transfer(), "open_from is inclusive");
    f.env.ledger().set_timestamp(200);
    assert!(f.can_transfer(), "open_until is inclusive");
}

#[test]
fn pause_overrides_open_window() {
    let f = setup();
    f.module.set_window(&Some(100), &Some(200));
    f.env.ledger().set_timestamp(150); // inside the window
    f.module.pause();
    assert!(!f.can_transfer(), "pause wins even inside an open window");
}

#[test]
#[should_panic]
fn set_window_requires_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let m = env.register(TransferWindowModule, (admin,));
    let module = TransferWindowModuleClient::new(&env, &m);
    module.set_window(&Some(1), &Some(2)); // no admin auth → must panic
}
