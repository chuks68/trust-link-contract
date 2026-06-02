use soroban_sdk::{contracttype, token, Address, Env, Vec};
use crate::{ContractError, EscrowData, ResolutionType};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferInstruction {
    pub recipient: Address,
    pub amount: i128,
}

pub fn execute_payout_transfers(
    env: &Env,
    token_addr: &Address,
    transfers: &Vec<TransferInstruction>,
) -> Result<(), ContractError> {
    let client = token::Client::new(env, token_addr);
    let contract_addr = env.current_contract_address();
    
    for instruction in transfers.iter() {
        if instruction.amount > 0 {
            client.transfer(&contract_addr, &instruction.recipient, &instruction.amount);
        }
    }
    Ok(())
}

/// Computes the protocol fee for `amount` at `fee_bps` basis points.
///
/// # Rounding policy: floor (round toward zero)
///
/// The fee is `floor(amount * fee_bps / 10_000)`. Integer division truncates,
/// so any sub-stroop remainder is dropped from the fee. Crucially, callers
/// derive the payout as `net = amount - fee` (see [`calculate_protocol_fee`]),
/// which means the truncated remainder is **not** lost — it stays in `net` and
/// is paid to the recipient (seller on release, buyer on refund). The contract
/// only ever retains exactly `fee`, which is later swept by the admin via
/// `withdraw_fees`. The invariant `net + fee == amount` therefore always holds
/// and no stroop is ever stranded in the vault.
///
/// Consequence of flooring: for amounts where `amount * fee_bps < 10_000` the
/// fee rounds down to `0` and is effectively waived. The `MIN_ESCROW_AMOUNT`
/// guard (1_000_000 stroops) in `create_escrow` keeps escrows large enough that
/// a non-zero `fee_bps` always yields a meaningful, non-zero fee.
///
/// Floor is chosen deliberately over ceiling/round-half-up: it guarantees the
/// contract never owes more than it custodies and never over-collects fees at
/// the recipient's expense.
///
/// The computation is split (`amount / 10_000 * fee_bps` plus the remainder
/// term) to avoid overflowing `i128` for large amounts.
pub fn calculate_fee(amount: i128, fee_bps: u32) -> Result<i128, ContractError> {
    if amount < 0 {
        return Err(ContractError::InvalidAmount);
    }

    let part1 = amount
        .checked_div(10_000)
        .ok_or(ContractError::ArithmeticOverflow)?
        .checked_mul(fee_bps as i128)
        .ok_or(ContractError::ArithmeticOverflow)?;

    let part2 = (amount % 10_000)
        .checked_mul(fee_bps as i128)
        .ok_or(ContractError::ArithmeticOverflow)?
        .checked_div(10_000)
        .ok_or(ContractError::ArithmeticOverflow)?;

    part1.checked_add(part2).ok_or(ContractError::ArithmeticOverflow)
}

pub fn calculate_protocol_fee(amount: i128, fee_bps: u32) -> Result<(i128, i128), ContractError> {
    let fee = calculate_fee(amount, fee_bps)?;
    let net = amount.checked_sub(fee).ok_or(ContractError::ArithmeticOverflow)?;
    Ok((fee, net))
}

pub fn calculate_dispute_allocations(
    env: &Env,
    escrow: &EscrowData,
    resolution: &ResolutionType,
    arbitration_fee: i128,
) -> Result<Vec<TransferInstruction>, ContractError> {
    if escrow.amount < arbitration_fee {
        return Err(ContractError::InsufficientBalance);
    }

    let remaining_amount = escrow.amount.checked_sub(arbitration_fee).ok_or(ContractError::ArithmeticOverflow)?;

    let (_fee, net_amount) = calculate_protocol_fee(remaining_amount, escrow.fee_bps)?;

    let recipient = match resolution {
        ResolutionType::Release => escrow.seller.clone(),
        ResolutionType::Refund => escrow.buyer.clone().ok_or(ContractError::EscrowHasNoBuyer)?,
    };

    let mut transfers = Vec::new(env);
    transfers.push_back(TransferInstruction {
        recipient,
        amount: net_amount,
    });

    Ok(transfers)
}
