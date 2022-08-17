use crate::*;

/// 1 HOUR in seconds
const MAX_GAME_DURATION_SEC: u32 = 60 * 60;
const MIN_GAME_DURATION_SEC: u32 = 100;
/// Max referrer fees - 50% equivalent in BASIS_P
const HALF_BASIS_P: u32 = BASIS_P / 2;

/// variables can be change after by owner
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    /// Service fee in BASIS_P E.g 10% => 1000; 2% => 200
    pub service_fee_percentage: u32,
    /// Referrer ratio to fees distribution from `service_fee_percentage`
    /// in BASIS_P. E.g if `service_fee_percentage` = 1000 (10%)
    /// `referrer_ratio` = 5000 means that 5% from total game reward
    /// comes to protocol and 5% to referrer
    pub referrer_ratio: u32,
    /// `max_game_duration_sec` in seconds (0..3600) is required 
    pub max_game_duration_sec: u32
}

impl Config {
    pub fn assert_valid(&self) {
        validate_fee(self.service_fee_percentage, self.referrer_ratio);
        validate_game_duration(self.max_game_duration_sec);
    }
}

pub (crate) fn validate_fee(service_fee: u32, referrer_fee: u32) {
    assert!(
        service_fee >= MIN_FEES && service_fee <= MAX_FEES, 
        "fees need to be in range 0.1..10%"
    );
    assert!(
        referrer_fee >= MIN_FEES && service_fee <= HALF_BASIS_P, 
        "fees need to be in range 0.1..50%"
    );
}
pub (crate) fn validate_game_duration(duration_sec: u32) {
    assert!(
        duration_sec >= MIN_GAME_DURATION_SEC,
        "max game duration must be more then 100 seconds"
    );
    assert!(
        duration_sec <= MAX_GAME_DURATION_SEC,
        "max game duration must be less then 1 hour in seconds ({})",
        MAX_GAME_DURATION_SEC
    )
}