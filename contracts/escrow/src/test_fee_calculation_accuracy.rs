#![cfg(test)]

use crate::helpers::payout::calculate_protocol_fee;
use crate::ContractError;

/// Parameterized test that verifies fee calculation is mathematically correct
/// for various fee_bps values: 0, 50, 100, 150, 200, 250, 300.
///
/// Acceptance Criteria:
/// - Each test case verifies vendor payout + fee = original amount (no rounding loss)
/// - 0 bps: fee is exactly 0
/// - 300 bps: fee is exactly 3% of amount
/// - Test includes amounts at minimum (1 stroop) and large values

#[test]
fn test_fee_calculation_0_bps_minimum_amount() {
    // 0 bps = 0% fee
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 0_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Verify fee is exactly 0
    assert_eq!(fee, 0);
    // Verify net + fee = original amount
    assert_eq!(net + fee, amount);
    // Verify net is the full amount
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_0_bps_large_amount() {
    // 0 bps = 0% fee
    let amount = 1_000_000_000_000_i128; // 1 trillion stroops
    let fee_bps = 0_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Verify fee is exactly 0
    assert_eq!(fee, 0);
    // Verify net + fee = original amount
    assert_eq!(net + fee, amount);
    // Verify net is the full amount
    assert_eq!(net, amount);
}

#[test]
fn test_fee_calculation_50_bps_minimum_amount() {
    // 50 bps = 0.5% fee
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 50_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // For 1 stroop: fee = 1 * 50 / 10000 = 0 (rounds down)
    assert_eq!(fee, 0);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_50_bps_large_amount() {
    // 50 bps = 0.5% fee
    let amount = 1_000_000_i128; // 1 million stroops
    let fee_bps = 50_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 1,000,000 * 0.005 = 5,000
    assert_eq!(fee, 5_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 995_000);
}

#[test]
fn test_fee_calculation_100_bps_minimum_amount() {
    // 100 bps = 1% fee
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 100_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // For 1 stroop: fee = 1 * 100 / 10000 = 0 (rounds down)
    assert_eq!(fee, 0);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_100_bps_large_amount() {
    // 100 bps = 1% fee
    let amount = 1_000_000_i128; // 1 million stroops
    let fee_bps = 100_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 1,000,000 * 0.01 = 10,000
    assert_eq!(fee, 10_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 990_000);
}

#[test]
fn test_fee_calculation_150_bps_minimum_amount() {
    // 150 bps = 1.5% fee
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 150_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // For 1 stroop: fee = 1 * 150 / 10000 = 0 (rounds down)
    assert_eq!(fee, 0);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_150_bps_large_amount() {
    // 150 bps = 1.5% fee
    let amount = 1_000_000_i128; // 1 million stroops
    let fee_bps = 150_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 1,000,000 * 0.015 = 15,000
    assert_eq!(fee, 15_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 985_000);
}

#[test]
fn test_fee_calculation_200_bps_minimum_amount() {
    // 200 bps = 2% fee
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 200_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // For 1 stroop: fee = 1 * 200 / 10000 = 0 (rounds down)
    assert_eq!(fee, 0);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_200_bps_large_amount() {
    // 200 bps = 2% fee
    let amount = 1_000_000_i128; // 1 million stroops
    let fee_bps = 200_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 1,000,000 * 0.02 = 20,000
    assert_eq!(fee, 20_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 980_000);
}

#[test]
fn test_fee_calculation_250_bps_minimum_amount() {
    // 250 bps = 2.5% fee
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 250_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // For 1 stroop: fee = 1 * 250 / 10000 = 0 (rounds down)
    assert_eq!(fee, 0);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_250_bps_large_amount() {
    // 250 bps = 2.5% fee
    let amount = 1_000_000_i128; // 1 million stroops
    let fee_bps = 250_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 1,000,000 * 0.025 = 25,000
    assert_eq!(fee, 25_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 975_000);
}

#[test]
fn test_fee_calculation_300_bps_minimum_amount() {
    // 300 bps = 3% fee (maximum allowed)
    let amount = 1_i128; // 1 stroop (minimum)
    let fee_bps = 300_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // For 1 stroop: fee = 1 * 300 / 10000 = 0 (rounds down)
    assert_eq!(fee, 0);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 1);
}

#[test]
fn test_fee_calculation_300_bps_large_amount() {
    // 300 bps = 3% fee (maximum allowed)
    let amount = 1_000_000_i128; // 1 million stroops
    let fee_bps = 300_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 1,000,000 * 0.03 = 30,000 (exactly 3%)
    assert_eq!(fee, 30_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 970_000);
}

#[test]
fn test_fee_calculation_300_bps_exact_percentage() {
    // Verify that 300 bps produces exactly 3% fee
    let amount = 10_000_000_i128; // 10 million stroops
    let fee_bps = 300_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_ok());
    
    let (fee, net) = result.unwrap();
    
    // Expected fee: 10,000,000 * 0.03 = 300,000 (exactly 3%)
    assert_eq!(fee, 300_000);
    // Verify net + fee = original amount (no rounding loss)
    assert_eq!(net + fee, amount);
    assert_eq!(net, 9_700_000);
}

#[test]
fn test_fee_calculation_no_rounding_loss_various_amounts() {
    // Test various amounts to ensure no rounding loss
    let test_cases = [
        (1_i128, 0_u32),
        (1_i128, 50_u32),
        (1_i128, 100_u32),
        (1_i128, 150_u32),
        (1_i128, 200_u32),
        (1_i128, 250_u32),
        (1_i128, 300_u32),
        (100_i128, 0_u32),
        (100_i128, 50_u32),
        (100_i128, 100_u32),
        (100_i128, 150_u32),
        (100_i128, 200_u32),
        (100_i128, 250_u32),
        (100_i128, 300_u32),
        (10_000_i128, 0_u32),
        (10_000_i128, 50_u32),
        (10_000_i128, 100_u32),
        (10_000_i128, 150_u32),
        (10_000_i128, 200_u32),
        (10_000_i128, 250_u32),
        (10_000_i128, 300_u32),
        (1_000_000_i128, 0_u32),
        (1_000_000_i128, 50_u32),
        (1_000_000_i128, 100_u32),
        (1_000_000_i128, 150_u32),
        (1_000_000_i128, 200_u32),
        (1_000_000_i128, 250_u32),
        (1_000_000_i128, 300_u32),
        (1_000_000_000_i128, 0_u32),
        (1_000_000_000_i128, 50_u32),
        (1_000_000_000_i128, 100_u32),
        (1_000_000_000_i128, 150_u32),
        (1_000_000_000_i128, 200_u32),
        (1_000_000_000_i128, 250_u32),
        (1_000_000_000_i128, 300_u32),
    ];
    
    for (amount, fee_bps) in test_cases {
        let result = calculate_protocol_fee(amount, fee_bps);
        assert!(result.is_ok(), "Failed for amount={}, fee_bps={}", amount, fee_bps);
        
        let (fee, net) = result.unwrap();
        
        // Critical: verify no rounding loss (net + fee must equal original amount)
        assert_eq!(
            net + fee,
            amount,
            "Rounding loss detected: amount={}, fee_bps={}, fee={}, net={}, sum={}",
            amount,
            fee_bps,
            fee,
            net,
            net + fee
        );
        
        // Verify fee is non-negative
        assert!(fee >= 0, "Fee cannot be negative: amount={}, fee_bps={}, fee={}", amount, fee_bps, fee);
        
        // Verify net is non-negative
        assert!(net >= 0, "Net cannot be negative: amount={}, fee_bps={}, net={}", amount, fee_bps, net);
    }
}

#[test]
fn test_fee_calculation_edge_case_amounts() {
    // Test edge cases with amounts that might cause rounding issues
    let edge_cases = [
        (9_999_i128, 300_u32),      // Just under 10,000
        (10_000_i128, 300_u32),     // Exactly 10,000
        (10_001_i128, 300_u32),     // Just over 10,000
        (99_999_i128, 300_u32),     // Just under 100,000
        (100_000_i128, 300_u32),    // Exactly 100,000
        (100_001_i128, 300_u32),    // Just over 100,000
        (999_999_i128, 300_u32),    // Just under 1,000,000
        (1_000_000_i128, 300_u32),  // Exactly 1,000,000
        (1_000_001_i128, 300_u32),  // Just over 1,000,000
    ];
    
    for (amount, fee_bps) in edge_cases {
        let result = calculate_protocol_fee(amount, fee_bps);
        assert!(result.is_ok(), "Failed for amount={}, fee_bps={}", amount, fee_bps);
        
        let (fee, net) = result.unwrap();
        
        // Verify no rounding loss
        assert_eq!(
            net + fee,
            amount,
            "Rounding loss at edge case: amount={}, fee_bps={}, fee={}, net={}",
            amount,
            fee_bps,
            fee,
            net
        );
    }
}

#[test]
fn test_fee_calculation_invalid_amount() {
    // Test that negative amounts return an error
    let amount = -1_i128;
    let fee_bps = 100_u32;
    
    let result = calculate_protocol_fee(amount, fee_bps);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::InvalidAmount);
}
