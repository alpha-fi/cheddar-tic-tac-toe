use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfig {
    pub(crate) token_id: TokenContractId,
    pub(crate) deposit: Balance,
    pub(crate) opponent_id: Option<AccountId>,
    pub(crate) referrer_id: Option<AccountId>,
    pub(crate) created_at: u64,
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfigView {
    pub(crate) token_id: TokenContractId,
    pub(crate) deposit: U128,
    pub(crate) opponent_id: Option<AccountId>,
    pub(crate) referrer_id: Option<AccountId>,
    pub(crate) created_at: u32,
}

impl From<&GameConfig> for GameConfigView {
    fn from(gc: &GameConfig) -> Self {
        Self { 
            token_id: gc.token_id.clone(), 
            deposit: gc.deposit.into(), 
            opponent_id: gc.opponent_id.clone(), 
            referrer_id: gc.referrer_id.clone(),
            created_at: nano_to_sec(gc.created_at)
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
    /// Only `token_id` and `deposit` on set
    pub fn with_only_token_params(
        token_id: &TokenContractId,
        deposit: Balance
    ) -> Self {
        Self { 
            token_id: token_id.clone(), 
            deposit, 
            opponent_id: None, 
            referrer_id: None,
            created_at: env::block_timestamp()
        }
    }
    /// `GameConfig` from transfer message
    pub fn from_transfer_msg(
        token_id: &TokenContractId,
        deposit: Balance,
        game_args: &GameConfigArgs
    ) -> Self {
        Self { 
            token_id: token_id.clone(), 
            deposit, 
            opponent_id: game_args.opponent_id.clone(), 
            referrer_id: game_args.referrer_id.clone(),
            created_at: env::block_timestamp()
        }
    }
}