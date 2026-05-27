#![cfg(test)]

use crate::{ResolutionType};
use soroban_sdk::{Env, TryFromVal, ConversionError};

#[test]
fn test_resolution_type_enum_serialization() {
    let env = Env::default();
    
    use soroban_sdk::Val;
    
    // Test Symbol -> Enum conversion
    // Enums with #[contracttype] can be converted from/to Val.
    // For variant conversion, we typically use Val or the generated client.
    let release_val = Val::try_from_val(&env, &0u32).unwrap();
    let release_enum = ResolutionType::try_from_val(&env, &release_val).unwrap();
    assert_eq!(release_enum, ResolutionType::Release);
    
    // Test Enum -> Val conversion
    let refund_val = Val::try_from_val(&env, &ResolutionType::Refund).unwrap();
    let refund_u32 = u32::try_from_val(&env, &refund_val).unwrap();
    assert_eq!(refund_u32, 1u32);
    
    // Test invalid value fails
    let invalid = Val::try_from_val(&env, &99u32).unwrap();
    let result: Result<ResolutionType, ConversionError> = ResolutionType::try_from_val(&env, &invalid);
    assert!(result.is_err());
}
