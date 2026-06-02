use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::{EscrowData, FeeConfig};

/// Typed keys for all contract storage entries.
///
/// Storage-tier rationale:
/// - Instance keys store singleton/global configuration and counters.
/// - Persistent keys store per-escrow data and user indexes that must survive
///   contract instance TTL changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    // Instance storage: global singleton values.
    /// Stores the current admin address responsible for contract administration.
    AdminAddress,
    /// Stores the global fee configuration (protocol_fee_bps, arbitration_fee_bps).
    FeeConfig,
    /// Stores the monotonically incrementing counter used to generate unique escrow IDs.
    EscrowCounter,

    // Persistent storage: large, append-only, or user-scoped records.
    /// Stores complete escrow data (seller, buyer, token, amount, state, etc.) by escrow ID.
    EscrowData(u64),
    /// Stores list of escrow IDs associated with a vendor (seller) address for easy lookup.
    VendorEscrowIndex(Address),
    /// Stores list of escrow IDs associated with a buyer address for easy lookup.
    BuyerEscrowIndex(Address),
}

pub fn write_admin_address(env: &Env, admin: &Address) {
    env.storage().instance().set(&StorageKey::AdminAddress, admin);
}

pub fn read_admin_address(env: &Env) -> Option<Address> {
    env.storage().instance().get(&StorageKey::AdminAddress)
}

pub fn write_fee_config(env: &Env, fee_config: &FeeConfig) {
    env.storage().instance().set(&StorageKey::FeeConfig, fee_config);
}

pub fn read_fee_config(env: &Env) -> Option<FeeConfig> {
    env.storage().instance().get(&StorageKey::FeeConfig)
}

pub fn write_escrow_counter(env: &Env, counter: u64) {
    env.storage().instance().set(&StorageKey::EscrowCounter, &counter);
}

pub fn read_escrow_counter(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&StorageKey::EscrowCounter)
        .unwrap_or(0)
}

pub fn write_escrow_data(env: &Env, escrow_id: u64, escrow: &EscrowData) {
    env.storage()
        .persistent()
        .set(&StorageKey::EscrowData(escrow_id), escrow);
}

pub fn read_escrow_data(env: &Env, escrow_id: u64) -> Option<EscrowData> {
    env.storage()
        .persistent()
        .get(&StorageKey::EscrowData(escrow_id))
}

pub fn write_vendor_escrow_index(env: &Env, vendor: &Address, escrow_ids: &Vec<u64>) {
    env.storage()
        .persistent()
        .set(&StorageKey::VendorEscrowIndex(vendor.clone()), escrow_ids);
}

pub fn read_vendor_escrow_index(env: &Env, vendor: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&StorageKey::VendorEscrowIndex(vendor.clone()))
        .unwrap_or(Vec::new(env))
}

pub fn write_buyer_escrow_index(env: &Env, buyer: &Address, escrow_ids: &Vec<u64>) {
    env.storage()
        .persistent()
        .set(&StorageKey::BuyerEscrowIndex(buyer.clone()), escrow_ids);
}

pub fn read_buyer_escrow_index(env: &Env, buyer: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&StorageKey::BuyerEscrowIndex(buyer.clone()))
        .unwrap_or(Vec::new(env))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn admin_and_counter_helpers_roundtrip() {
        let env = Env::default();
        let admin = Address::generate(&env);

        write_admin_address(&env, &admin);
        write_escrow_counter(&env, 42);

        assert_eq!(read_admin_address(&env), Some(admin));
        assert_eq!(read_escrow_counter(&env), 42);
    }

    #[test]
    fn vendor_and_buyer_index_helpers_roundtrip() {
        let env = Env::default();
        let vendor = Address::generate(&env);
        let buyer = Address::generate(&env);

        let mut vendor_ids = Vec::new(&env);
        vendor_ids.push_back(1);
        vendor_ids.push_back(7);

        let mut buyer_ids = Vec::new(&env);
        buyer_ids.push_back(2);
        buyer_ids.push_back(9);

        write_vendor_escrow_index(&env, &vendor, &vendor_ids);
        write_buyer_escrow_index(&env, &buyer, &buyer_ids);

        assert_eq!(read_vendor_escrow_index(&env, &vendor), vendor_ids);
        assert_eq!(read_buyer_escrow_index(&env, &buyer), buyer_ids);
    }
}
