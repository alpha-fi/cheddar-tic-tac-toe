use crate::*;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;

#[ext_contract(ext_ft)]
pub trait ExtFungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[derive(Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GameConfigArgs {
    pub opponent_id: Option<AccountId>,
    pub referrer_id: Option<AccountId>,
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
        assert!(
            token_id == self.cheddar,
            "Only cheddar {} is accepted for deposits",
            self.cheddar
        );
        assert!(
            amount.0 >= self.min_deposit,
            "deposited amount must be more than {}",
            self.min_deposit
        );
        log!("deposit {} cheddar from @{}", amount.0, sender_id);
        let available_complete = self.make_deposit(&sender_id, amount.0);

        if available_complete {
            PromiseOrValue::Value(U128(0))
        } else {
            PromiseOrValue::Value(amount)
        }
    }
}

impl Contract {
    pub(crate) fn make_deposit(
        &mut self,
        sender_id: &AccountId,
        amount: Balance,
    ) -> bool
    {
        if self.is_user_registered(sender_id) {
            self.make_deposit(sender_id, amount);
        } else {
            return false;
        }
        true

    }
}
