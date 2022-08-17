use crate::*;
use near_contract_standards::{
    fungible_token::receiver::FungibleTokenReceiver,
};

#[ext_contract(ext_ft)]
pub trait ExtFungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[derive(Deserialize, Serialize)]
#[serde(crate="near_sdk::serde")]
pub struct GameConfigArgs {
    pub opponent_id: Option<AccountId>,
    pub referrer_id: Option<AccountId>
}

/// FT Receiver
/// token deposits are done through NEP-141 ft_transfer_call to the contract.
#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// FungibleTokenReceiver implementation Callback on receiving tokens by this contract.
    /// Handles both farm deposits and stake deposits. For farm deposit (sending tokens
    /// to setup the farm) you must set "setup reward deposit" msg.
    /// Otherwise tokens will be staken.
    /// Returns zero.
    /// Panics when:
    /// - account is not registered
    /// - or receiving a wrong token
    /// - or making a farm deposit after farm is finalized
    /// - or staking before farm is finalized.
    #[allow(unused_variables)]
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let token_id = env::predecessor_account_id();

        let min_deposit = match self.min_deposit(&token_id) {
            Some(amount) => amount,
            None => panic!("Token {} is not whitelisted", &token_id)
        };

        assert!(
            amount.0 >= min_deposit, 
            "deposited amount must be more than {}",
            min_deposit
        );
        
        let game_config = if msg.is_empty() {
            GameConfig::with_only_token_params(&token_id, amount.0)
        } else {
            let game_args:GameConfigArgs = near_sdk::serde_json::from_str(&msg).expect("Config is invalid");
            GameConfig::from_transfer_msg(&token_id, amount.0, &game_args)
        };

        log!("in deposit from @{} with token: {} amount {} ", sender_id, token_id, amount.0);

        let available_complete = self.internal_make_available(
            game_config,
            &sender_id,
        );
        
        if available_complete {
            PromiseOrValue::Value(U128(0))
        } else {
            PromiseOrValue::Value(amount)
        }
    }
}

impl Contract {
    pub (crate) fn internal_make_available(
        &mut self,
        game_config: GameConfig, 
        sender_id: &AccountId,
    ) -> bool {
        let amount = game_config.deposit;
        let token_id = game_config.token_id;
        let referrer_id:Option<AccountId> = game_config.referrer_id.clone();
        assert!(self.available_players.get(&sender_id).is_none(), "Already in the waiting list the list");
        
        //create config
        self.available_players.insert(&sender_id,
            &GameConfig {
                token_id: token_id.clone(),
                deposit: amount,
                opponent_id: game_config.opponent_id,
                referrer_id,
                created_at: env::block_timestamp()
            }
        );
        
        self.internal_check_player_available(&sender_id);
        if let Some(referrer_id) = game_config.referrer_id {
            self.internal_add_referrer(&sender_id, &referrer_id);
        }
        log!("Success deposit from @{} with {} of `{}` ", sender_id, amount, token_id);
        true 
    }
    /// getting min deposit to check it on FT Receiver
    /// returns None if token isn't whitelisted
    pub (crate) fn min_deposit(&self, token_id: &TokenContractId) -> Option<Balance> {
        match self.whitelisted_tokens.get(&token_id) {
            Some(min_deposit) => {
                Some(min_deposit)
            },
            None => None,
        }
    }
}