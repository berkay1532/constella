#![no_std]
//! ZK-eligibility compliance module.
//!
//! Gates transfers on the recipient's **ZK eligibility flag** (`is_verified`) from a
//! ZK identity provider (`module-identity-zk`) — NOT on a cleartext country. So a
//! disallowed recipient simply shows up as "not eligible"; their country is never read
//! or revealed. This is the privacy win over the cleartext `CountryRestrict` module.
//!
//! (Checks the recipient `to`; a production gate would check both parties.)

use soroban_sdk::{contract, contractclient, contractimpl, contracttype, Address, Env};

/// Minimal client for the ZK identity provider.
#[contractclient(name = "IdentityZkClient")]
pub trait IdentityZk {
    fn is_verified(env: Env, account: Address) -> bool;
}

#[contracttype]
enum DataKey {
    Admin,
    Identity,
}

#[contract]
pub struct ZkEligibilityModule;

#[contractimpl]
impl ZkEligibilityModule {
    pub fn __constructor(env: Env, admin: Address, identity: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Identity, &identity);
    }

    pub fn identity(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Identity).unwrap()
    }

    pub fn can_transfer(env: Env, _from: Address, to: Address, _amount: i128, _token: Address) -> bool {
        Self::eligible(&env, &to)
    }

    pub fn can_create(env: Env, to: Address, _amount: i128, _token: Address) -> bool {
        Self::eligible(&env, &to)
    }

    pub fn transferred(_env: Env, _from: Address, _to: Address, _amount: i128, _token: Address) {}
    pub fn created(_env: Env, _to: Address, _amount: i128, _token: Address) {}
    pub fn destroyed(_env: Env, _from: Address, _amount: i128, _token: Address) {}
}

impl ZkEligibilityModule {
    fn eligible(env: &Env, who: &Address) -> bool {
        let id: Address = env.storage().instance().get(&DataKey::Identity).unwrap();
        IdentityZkClient::new(env, &id).is_verified(who)
    }
}
