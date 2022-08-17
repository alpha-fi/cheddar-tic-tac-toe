use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfig {
    pub(crate) token_id: TokenContractId,
    pub(crate) deposit: Balance,
    pub(crate) opponent_id: Option<AccountId>,
    pub(crate) referrer_id: Option<AccountId>
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfigNear {
    pub(crate) deposit: Balance,
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
            referrer_id: None 
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
            referrer_id: game_args.referrer_id.clone() 
        }
    }
}