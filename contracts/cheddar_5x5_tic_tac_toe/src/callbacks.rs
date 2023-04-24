use crate::*;

#[near_bindgen]
impl Contract {
    #[private]
    pub fn transfer_deposit_callback(&mut self, user: AccountId, config: &GameConfig) {
        if promise_result_as_failed() {
            log!(
                "transfer available deposit {} of {} token failed. recovering @{} state",
                config.deposit,
                self.cheddar,
                user.clone()
            );
            self.available_players.insert(&user, config);
        }
    }
    #[private]
    pub fn transfer_callback(&mut self, user: AccountId, stats: &StatsView) {
        if promise_result_as_failed() {
            log!(
                "Transfer failed. Recovering state for {} account",
                user.clone(),
            );
            let stats = Stats  {
                    referrer_id: stats.referrer_id.clone(),
                    affiliates: UnorderedSet::new(b"s"), //TODO: not sure why we store it as unordered set
                    games_num: stats.games_played,
                    victories_num: stats.victories_num,
                    penalties_num: stats.penalties_num,
                    total_reward: stats.total_reward,
                    total_affiliate_reward: stats.total_affiliate_reward,
            };
            self.stats.insert(&user.clone(), &stats);
        }
    }
    #[private]
    pub fn cheddar_withdraw_callback(&mut self, user: &AccountId, vault: Vault) {
        if promise_result_as_failed() {
            log!(
                "FT withdrawal failed. Recovering state for {} account",
                user.clone(),
            );
            self.registered_players.insert(user, &vault);
        }
    }
}