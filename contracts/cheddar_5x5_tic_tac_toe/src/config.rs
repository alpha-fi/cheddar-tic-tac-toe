use crate::*;

/// 4 HOURS in seconds
pub (crate) const MAX_GAME_DURATION_SEC: u64 = 4 * 60 * 60;
/// 25 MINUTES in seconds
const MIN_MAX_GAME_DURATION_SEC: u64 = 25 * 60;

/// variables can be change after by owner
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    /// Service fee in basis points E.g 2% => 200; 10% => 1000
    pub fee: u16,
    /// Referrer ratio to fees distribution from `service_fee_percentage`
    /// in BASIS_P. E.g if `service_fee_percentage` = 1000 (10%)
    /// `referrer_ratio` = 5000 means that 5% from total game reward
    /// comes to protocol and 5% to referrer
    pub referrer_fee_share: u16,
    /// `max_game_duration_sec` in seconds (0..3600) is required
    pub max_game_duration_sec: u64,
    /// max number of stored games into contract
    pub max_stored_games: u8,
}

impl Config {
    pub fn assert_valid(&self) {
        validate_fee(self.fee, self.referrer_fee_share);
        validate_game_duration(self.max_game_duration_sec);
    }
}

pub(crate) fn validate_fee(service_fee: u16, referrer_fee_share: u16) {
    assert!(service_fee <= MAX_FEES, "fees must be in range 0..10%");
    assert!(
        referrer_fee_share <= BASIS_P,
        "referrer fees need to be in range 0..10000 from total fees"
    );
}
pub(crate) fn validate_game_duration(d: u64) {
    assert!(
        MIN_MAX_GAME_DURATION_SEC <= d && d <= MAX_GAME_DURATION_SEC,
        "max game duration must be between {} and {}sec",
        MIN_MAX_GAME_DURATION_SEC,
        MAX_GAME_DURATION_SEC
    );
}
