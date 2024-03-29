use std::convert::TryInto;

use crate::*;

#[allow(unused)]
pub(crate) const NO_DEPOSIT: u128 = 0;
pub(crate) const CALLBACK_GAS: Gas = Gas(Gas::ONE_TERA.0 * 5);
pub(crate) const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);

pub(crate) const MAX_FEES: u16 = 500; // 5%
pub(crate) const BASIS_P: u16 = 10000; // 100%
pub(crate) const TIMEOUT_WIN: Duration = 5 * 60; // 5 minutes timeout in seconds
pub(crate) const MIN_DEPOSIT_CHEDDAR: Balance =  50;
pub(crate) const MIN_AVAILABLE_FOR: Duration = 1 * 60; // 1 minute 
pub(crate) const MAX_AVAILABLE_FOR: Duration = 60 * 60; // 1 hour 
pub(crate) const MIN_BET_CHEDDAR: Balance = 50;
 
pub(crate) type GameId = u64;
pub(crate) type AffiliateId = AccountId;

/// This constant can be used to set the board size
pub(crate) const BOARD_SIZE: u8 = 25;
pub(crate) const MAX_NUM_TURNS: u64 = BOARD_SIZE as u64 * BOARD_SIZE as u64;

/// pesimistic assumption of the storage_deposit needed for every user 
pub(crate) const STORAGE_COST_PER_USER: Balance = 200_000_000_000_000_000_000_000; // 0.2 NEAR in YOCTONEAR


/// Returns true if the promise was failed. Otherwise returns false.
/// Fails if called outside a callback that received 1 promise result.
pub(crate) fn promise_result_as_failed() -> bool {
    require!(
        env::promise_results_count() == 1,
        "Contract expected a result on the callback"
    );
    match env::promise_result(0) {
        PromiseResult::Failed => true,
        _ => false,
    }
}

pub(crate) fn sec_to_nano(sec: u64) -> Duration {
    u64::from(sec) * 10u64.pow(9)
}

pub(crate) fn nano_to_sec(nano: Duration) -> Duration {
    match nano.checked_div(10u64.pow(9)) {
        Some(sec) => sec.try_into().unwrap(),
        None => panic!("Math error while converting nano to sec"),
    }
}
