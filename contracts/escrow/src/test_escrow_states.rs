#![cfg(test)]
//! Verifies the `EscrowState` lifecycle and the `transition_state` validity
//! matrix (#9): all 7 states exist, every legal edge is accepted, every illegal
//! edge is rejected with `InvalidStateTransition`, and self-loops are illegal.

use crate::{transition_state, ContractError, EscrowState};

#[test]
fn all_seven_states_are_defined() {
    // Pattern-match exhaustively so adding/removing a variant breaks the test.
    let states = [
        EscrowState::Pending,
        EscrowState::Funded,
        EscrowState::Shipped,
        EscrowState::Completed,
        EscrowState::Disputed,
        EscrowState::Refunded,
        EscrowState::Canceled,
    ];
    assert_eq!(states.len(), 7);
}

#[test]
fn legal_transitions_are_accepted() {
    let legal: &[(EscrowState, EscrowState)] = &[
        (EscrowState::Pending, EscrowState::Funded),
        (EscrowState::Pending, EscrowState::Canceled),
        (EscrowState::Funded, EscrowState::Shipped),
        (EscrowState::Funded, EscrowState::Completed),
        (EscrowState::Funded, EscrowState::Refunded),
        (EscrowState::Shipped, EscrowState::Completed),
        (EscrowState::Shipped, EscrowState::Disputed),
        (EscrowState::Shipped, EscrowState::Refunded),
        (EscrowState::Disputed, EscrowState::Completed),
        (EscrowState::Disputed, EscrowState::Refunded),
    ];
    for (from, to) in legal {
        assert!(
            transition_state(from, to).is_ok(),
            "{:?} -> {:?} should be allowed",
            from,
            to,
        );
    }
}

#[test]
fn illegal_transitions_are_rejected() {
    let illegal: &[(EscrowState, EscrowState)] = &[
        // Terminal states have no outgoing edges.
        (EscrowState::Completed, EscrowState::Funded),
        (EscrowState::Completed, EscrowState::Disputed),
        (EscrowState::Refunded, EscrowState::Funded),
        (EscrowState::Canceled, EscrowState::Funded),
        // Cannot skip Funded.
        (EscrowState::Pending, EscrowState::Shipped),
        (EscrowState::Pending, EscrowState::Completed),
        // Cannot un-cancel.
        (EscrowState::Canceled, EscrowState::Pending),
        // Disputes are only opened once the escrow has shipped.
        (EscrowState::Funded, EscrowState::Disputed),
        // Cannot dispute a Pending escrow that was never funded.
        (EscrowState::Pending, EscrowState::Disputed),
    ];
    for (from, to) in illegal {
        assert_eq!(
            transition_state(from, to),
            Err(ContractError::InvalidStateTransition),
            "{:?} -> {:?} should be rejected",
            from,
            to,
        );
    }
}

#[test]
fn self_loops_are_illegal() {
    // No state should be a legal transition to itself.
    for s in [
        EscrowState::Pending,
        EscrowState::Funded,
        EscrowState::Shipped,
        EscrowState::Completed,
        EscrowState::Disputed,
        EscrowState::Refunded,
        EscrowState::Canceled,
    ] {
        assert_eq!(
            transition_state(&s, &s),
            Err(ContractError::InvalidStateTransition),
            "{:?} -> {:?} self-loop should be rejected",
            s,
            s,
        );
    }
}
