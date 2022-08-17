use std::convert::TryInto;

use crate::*;

pub (crate) const MAX_TIME_TO_BE_AVAILABLE: u64 = 24 * 60 * 60 * 1_000_000_000; // 1day in nanoseconds

#[allow(unused)]
pub(crate) const NO_DEPOSIT:u128 = 0;
pub(crate) const CALLBACK_GAS: Gas = Gas(Gas::ONE_TERA.0 * 5);
pub(crate) const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);

pub(crate) const MIN_FEES: u32 = 10;   // 0.1%
pub(crate) const MAX_FEES: u32 = 1000; // 10%
pub(crate) const BASIS_P: u32 = 10000; // 100%

pub(crate) const MIN_DEPOSIT_NEAR: Balance = ONE_NEAR / 10; // 0.1 NEAR


pub(crate) type TokenContractId = AccountId;
pub(crate) type GameId = u64;
pub(crate) type AffiliateId = AccountId;

/// This constant can be used to set the board size
pub(crate) const BOARD_SIZE: usize = 5;
pub(crate) const MAX_NUM_TURNS: u64 = 25;
pub(crate) const PLAYERS_NUM: usize = 2;

/// Returns true if the promise was failed. Otherwise returns false.
/// Fails if called outside a callback that received 1 promise result.
pub (crate) fn promise_result_as_failed() -> bool {
    require!(env::promise_results_count() == 1, "Contract expected a result on the callback");
    match env::promise_result(0) {
        PromiseResult::Failed => true,
        _ => false,
    }
}

pub (crate) fn sec_to_nano(sec: u32) -> Duration {
    u64::from(sec) * 10u64.pow(9)
}

pub (crate) fn nano_to_sec(nano: Duration) -> u32 {
    match nano.checked_div(10u64.pow(9)) {
        Some(sec) => sec.try_into().unwrap(),
        None => panic!("Math error while converting nano to sec")
    }
}