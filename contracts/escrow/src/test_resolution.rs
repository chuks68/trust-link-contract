#![cfg(test)]

use crate::ResolutionType;
use soroban_sdk::{ConversionError, Env, IntoVal, TryFromVal, Val};

#[test]
fn test_resolution_type_enum_serialization() {
    let env = Env::default();

    // Test Enum -> xdr conversion (round-trip via contracttype binary encoding)
    let release_val: Val = ResolutionType::Release.into_val(&env);
    let release_back = ResolutionType::try_from_val(&env, &release_val).unwrap();
    assert_eq!(release_back, ResolutionType::Release);

    let refund_val: Val = ResolutionType::Refund.into_val(&env);
    let refund_back = ResolutionType::try_from_val(&env, &refund_val).unwrap();
    assert_eq!(refund_back, ResolutionType::Refund);

    // Test invalid value fails
    let invalid: Val = Val::from_u32(99).into();
    let result: Result<ResolutionType, ConversionError> =
        ResolutionType::try_from_val(&env, &invalid);
    assert!(result.is_err());
}
