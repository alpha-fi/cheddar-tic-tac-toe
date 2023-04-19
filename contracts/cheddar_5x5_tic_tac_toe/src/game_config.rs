use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfig {
    pub(crate) deposit: Balance,
    pub(crate) opponent_id: Option<AccountId>,
    pub(crate) referrer_id: Option<AccountId>,
    pub(crate) created_at: Timestamp, // timestamp in seconds
    pub(crate) available_to: Timestamp, 
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfigView {
    pub(crate) deposit: U128,
    pub(crate) opponent_id: Option<AccountId>,
    pub(crate) referrer_id: Option<AccountId>,
    pub(crate) created_at: Timestamp,
}

impl From<&GameConfig> for GameConfigView {
    fn from(gc: &GameConfig) -> Self {
        Self { 
            deposit: gc.deposit.into(), 
            opponent_id: gc.opponent_id.clone(), 
            referrer_id: gc.referrer_id.clone(),
            created_at: nano_to_sec(gc.created_at).into()
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfigNear {
    pub(crate) opponent_id: Option<AccountId>,
    pub(crate) referrer_id: Option<AccountId>
}

impl GameConfig {
    /// Empty transfer message
    pub fn with_only_token_params(
        deposit: Balance
    ) -> Self {
        Self { 
            deposit, 
            opponent_id: None, 
            referrer_id: None,
            created_at: nano_to_sec(env::block_timestamp()).into(),
            available_to: 0,
        }
    }
    /// `GameConfig` from transfer message
    pub fn from_transfer_msg(
        deposit: Balance,
        game_args: &GameConfigArgs
    ) -> Self {
        Self {  
            deposit, 
            opponent_id: game_args.opponent_id.clone(), 
            referrer_id: game_args.referrer_id.clone(),
            created_at: nano_to_sec(env::block_timestamp()).into(),
            available_to: 0,
        }
    }
}