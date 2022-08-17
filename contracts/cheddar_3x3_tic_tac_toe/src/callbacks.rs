use crate::*;

#[near_bindgen]
impl Contract {
    #[private]
    pub fn transfer_deposit_callback(&mut self, user: AccountId, config: &GameConfig) {
        if promise_result_as_failed() {
            log!(
                "transfer available deposit {} of {} token failed. recovering @{} state",
                config.deposit,
                config.token_id,
                user.clone()
            );
            self.available_players.insert(&user, config);
        }
    }
}