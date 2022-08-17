use crate::*;

#[near_bindgen]
impl Contract {
    /// Decimals must be set accurate because of counting min deposit!
    #[private]
    pub fn whitelist_token(&mut self, token_id: TokenContractId, min_deposit: U128) {
        assert!(self.whitelisted_tokens.insert(&token_id, &min_deposit.0).is_none());
    }
    /// set accuracy, service fees need to be in range [0.1..10%]
    /// also referrer_fee need to be [0..50%] from service fee
    #[private]
    pub fn set_service_fee(&mut self, service_fee: u32, referrer_fee: u32) -> bool {
        validate_fee(service_fee, referrer_fee);
        self.service_fee_percentage = service_fee;
        self.referrer_ratio = referrer_fee;
        true
    }
    /// set accuracy, max_duration need to be in range [100..3600] seconds
    #[private]
    pub fn set_max_duration(&mut self, max_duration: u32) -> bool {
        validate_game_duration(max_duration);
        self.max_game_duration = sec_to_nano(max_duration);
        true
    }
}

impl Contract {
    pub (crate) fn internal_transfer(
        &mut self,
        token_id: &TokenContractId,
        receiver_id: &AccountId,
        amount: U128
    ) -> Promise {
        if token_id == &AccountId::new_unchecked("near".into()) {
            Promise::new(receiver_id.clone()).transfer(amount.0)
        } else {
            ext_ft::ext(token_id.clone())
                .with_static_gas(GAS_FOR_FT_TRANSFER)
                .with_attached_deposit(ONE_YOCTO)
                .ft_transfer(receiver_id.clone(), amount, None)
        }
    }
    pub (crate) fn internal_distribute_reward(
        &mut self,
        game_id: &GameId,
        winner: Option<&AccountId>,
    )  -> Balance {
        let reward = self.internal_get_game_reward(game_id);
        let players_deposit = reward.balance;
        let token_id = reward.token_id.clone();
        let fees_amount = players_deposit
            .checked_div(BASIS_P.into())
            .unwrap_or(0)
            .checked_mul(self.service_fee_percentage as u128)
            .unwrap_or(0);
        assert!(fees_amount > 0, "Incorrect fees computing");

        let winner_reward: Balance = players_deposit - fees_amount;

        if let Some(winner_id) = winner {
            log!("Winner is {}. Reward: {}", winner_id, winner_reward);

            self.internal_transfer(&token_id, winner_id, winner_reward.into());

            self.internal_distribute_fee(&token_id, fees_amount, winner_id);
            self.internal_update_stats(
                Some(&token_id), 
                winner_id, 
                UpdateStatsAction::AddWonGame, 
                None, 
                None
            );
            self.internal_update_stats(
                Some(&token_id), 
                winner_id, 
                UpdateStatsAction::AddTotalReward, 
                None, 
                Some(winner_reward)
            );
            winner_reward
        } else {
            let refund_amount = match winner_reward.checked_div(PLAYERS_NUM as u128) {
                Some(amount) => amount,
                None => panic!("Failed divide deposit to refund GameDeposit (GameResult::Tie)")
            };
            assert!(
                refund_amount.checked_mul(PLAYERS_NUM as u128) < Some(reward.balance),
                "Incorrect Tie refund amount calculation"
            );
            log!("Tie. Refund: {}", refund_amount);
            self.internal_tie_refund(
                game_id, 
                &token_id, 
                refund_amount
            );
            refund_amount
        }
    }

    pub (crate) fn internal_distribute_fee(
        &mut self,
        token_id: &TokenContractId,
        service_fee: Balance,
        account_id: &AccountId
    ) -> Balance {
        // potential referrer fee
        let stats = self.internal_get_stats(account_id);
        let referrer_fee = if let Some(referrer_id) = stats.referrer_id {
            let computed_referrer_fee = service_fee
                .checked_div(BASIS_P.into())
                .unwrap_or(0)
                .checked_mul(self.referrer_ratio as u128)
                .unwrap_or(0);
            log!("Affiliate reward for @{} is {}", referrer_id, computed_referrer_fee);
            self.internal_update_stats(
                Some(token_id), 
                &referrer_id, 
                UpdateStatsAction::AddAffiliateReward, 
                None, 
                Some(computed_referrer_fee)
            );
            // transfer fee to referrer
            self.internal_transfer(&token_id, &referrer_id, computed_referrer_fee.into());

            computed_referrer_fee
        } else {
            0
        };

        assert!(
            service_fee / 2 >= referrer_fee,
            "Referrer fees cannot be more then 50% from service fee"
        );
        referrer_fee
    }

    pub (crate) fn internal_tie_refund(
        &mut self, 
        game_id: &GameId, 
        token_id: &TokenContractId, 
        refund_amount: Balance
    ) {
        let (player1, player2) = self.internal_get_game_players(game_id);
        self.internal_transfer(&token_id, &player1, refund_amount.into());
        self.internal_transfer(&token_id, &player2, refund_amount.into());
    }

    pub (crate) fn internal_stop_expired_game(&mut self, game_id: &GameId, looser: AccountId) {
        let mut game: Game = self.internal_get_game(&game_id);
        assert_eq!(game.game_state, GameState::Active, "Current game isn't active");
        
        self.internal_update_stats(
            None, 
            &looser, 
            UpdateStatsAction::AddPenaltyGame, 
            None, 
            None);
        
        let (player1, player2) = self.internal_get_game_players(game_id);
        
        let winner = if looser == player1{
            player2
        } else if looser == player2 {
            player1
        } else {
            panic!("Account @{} not in this game. GameId: {} ", looser, game_id)
        };

        self.internal_distribute_reward(game_id, Some(&winner));
        game.change_state(GameState::Finished);
        self.internal_update_game(game_id, &game);
        self.internal_stop_game(game_id);
    }

    pub(crate) fn is_account_exists(&self, account_id: &AccountId) -> bool {
        if let Some(_stats) = self.stats.get(account_id) {
            true
        } else {
            false
        }
    }

    pub (crate) fn internal_check_player_available(&mut self, account_id: &AccountId) {
        let has_games_started = self.games
            .iter()
            .any(|(_game_id, game)| game.contains_player_account_id(account_id));
        assert!(!has_games_started, "Player @{} already start another game", &account_id)
    }

    pub (crate) fn internal_add_referrer(&mut self, player_id: &AccountId, referrer_id: &AccountId) {
        if self.stats.get(player_id).is_none() && self.is_account_exists(referrer_id) {
            self.internal_update_stats(None, player_id, UpdateStatsAction::AddReferral, Some(referrer_id.clone()), None);
            self.internal_update_stats(None, referrer_id, UpdateStatsAction::AddAffiliate, Some(player_id.clone()), None);
            log!("Referrer {} added for {}", referrer_id, player_id);
        } else {
            log!("Referrer was not added")
        }
    }

    pub (crate) fn internal_get_game(&self, game_id: &GameId) -> Game {
        self.games
            .get(game_id)
            .expect("Game not found")
    }
    pub (crate) fn internal_stop_game(&mut self, game_id: &GameId) {
        let game = self.games
            .get(game_id)
            .expect("Game not found");
        assert_eq!(game.game_state, GameState::Finished, "Cannot stop. Game in progress");
        self.games.remove(game_id);
    }
    pub (crate) fn internal_get_game_players(&self, game_id: &GameId) -> (AccountId, AccountId) {
        let game = self.internal_get_game(game_id);
        game.get_player_accounts()
    }
    pub(crate) fn internal_get_game_reward(&self, game_id: &GameId) -> GameDeposit {
        let game = self.internal_get_game(game_id);
        game.reward()
    }
}