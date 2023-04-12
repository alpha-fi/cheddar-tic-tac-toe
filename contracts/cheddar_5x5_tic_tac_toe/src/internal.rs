use crate::*;

#[near_bindgen]
impl Contract {
    #[private]
    pub fn set_service_fee(&mut self, service_fee: u16, referrer_fee: u16) -> bool {
        validate_fee(service_fee, referrer_fee);
        self.service_fee = service_fee;
        self.referrer_fee_share = referrer_fee;
        true
    }

    /// set accuracy, max_duration need to be in range [100..3600] seconds
    #[private]
    pub fn set_max_duration(&mut self, max_duration: u64) -> bool {
        validate_game_duration(max_duration);
        self.max_game_duration = max_duration.into();
        self.max_turn_duration = self.max_game_duration / MAX_NUM_TURNS;
        true
    }
}

impl Contract {
    pub(crate) fn internal_get_available_player(&self, account_id: &AccountId) -> GameConfig {
        self.available_players
            .get(account_id)
            .expect("You are not in available players list!")
    }

    pub(crate) fn internal_ping_expired_games(&mut self, ts: u64) {
        let expired_games_ids: Vec<GameId> = self
            .games
            .iter()
            .filter(|(_, game)| ts - game.initiated_at > self.max_game_duration_sec)
            .map(|(game_id, _)| game_id)
            .collect();
        if !expired_games_ids.is_empty() {
            for game_id in expired_games_ids.iter() {
                let game = self.internal_get_game(game_id);
                self.internal_stop_expired_game(game_id, game.current_player_account_id());
                log!(
                    "GameId: {}. Game duration expired. Required:{} Current:{} ",
                    game_id,
                    self.max_game_duration_sec,
                    ts - game.initiated_at
                );
            }
        }
        self.last_update_timestamp = nano_to_sec(ts);
    }

    pub(crate) fn internal_ping_expired_players(&mut self, ts: u64) {
        let current_timestamp:Timestamp = nano_to_sec(ts);
        let expired_players: Vec<(AccountId, GameConfig)> = self
            .available_players
            .iter()
            .filter(|(_, config)| current_timestamp - config.created_at > MAX_TIME_TO_BE_AVAILABLE)
            .map(|(account_id, config)| (account_id.clone(), config))
            .collect();
        if !expired_players.is_empty() {
            for (account_id, config) in expired_players.iter() {
                let token_id = config.token_id.clone();
                self.available_players.remove(&account_id);
                self.internal_transfer(&token_id, &account_id, config.deposit.into())
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(CALLBACK_GAS)
                            .transfer_deposit_callback(account_id.clone(), config),
                    );
                log!(
                    "Remove expired player @{}, refund {} of {}",
                    account_id,
                    config.deposit,
                    config.token_id
                );
            }
        }
        self.last_update_timestamp = nano_to_sec(ts);
    }

    pub(crate) fn internal_transfer(
        &mut self,
        token_id: &TokenContractId,
        receiver_id: &AccountId,
        amount: U128,
    ) -> Promise {
        if token_id == &AccountId::new_unchecked("near".into()) {
            Promise::new(receiver_id.clone()).transfer(amount.0)
        } else {
            ext_ft::ext(token_id.clone())
                .with_static_gas(GAS_FOR_FT_TRANSFER)
                .with_attached_deposit(ONE_YOCTO)
                .ft_transfer(receiver_id.clone(), amount, None)
            // TODO (finish): .then(callback to check the transfer)
        }
    }

    pub(crate) fn internal_distribute_reward(
        &mut self,
        game_id: &GameId,
        winner: Option<&AccountId>,
    ) -> U128 {
        let reward = self.internal_get_game_reward(game_id);
        let players_deposit = reward.balance;
        let token_id = reward.token_id.clone();
        let fees_amount = players_deposit
            .0
            .checked_div(BASIS_P.into())
            .unwrap_or(0)
            .checked_mul(self.service_fee as u128)
            .expect("multiplication overflow");

        let winner_reward: Balance = players_deposit.0 - fees_amount;

        if let Some(winner_id) = winner {
            log!("Winner is {}. Reward: {}", winner_id, winner_reward);
            let stats = self.get_stats(winner_id);
            self.internal_transfer(&token_id, winner_id, winner_reward.into())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .transfer_callback(winner_id.clone(), &stats),
            );

            self.internal_distribute_fee(&token_id, fees_amount, winner_id);
            self.internal_update_stats(
                Some(&token_id),
                winner_id,
                UpdateStatsAction::AddWonGame,
                None,
                None,
            );
            self.internal_update_stats(
                Some(&token_id),
                winner_id,
                UpdateStatsAction::AddTotalReward,
                None,
                Some(winner_reward),
            );
            winner_reward.into()
        } else {
            let refund_amount = match winner_reward.checked_div(2) {
                Some(amount) => amount,
                None => panic!("Failed divide deposit to refund GameDeposit (GameResult::Tie)"),
            };
            require!(
                refund_amount.checked_mul(2) <= Some(reward.balance.0),
                "Incorrect Tie refund amount calculation"
            );
            log!("Tie. Refund: {}", refund_amount);
            self.internal_tie_refund(game_id, &token_id, refund_amount);
            refund_amount.into()
        }
    }

    pub(crate) fn internal_distribute_fee(
        &mut self,
        token_id: &TokenContractId,
        service_fee: Balance,
        account_id: &AccountId,
    ) -> Balance {
        // potential referrer fee
        let stats = self.internal_get_stats(account_id);
        let referrer_fee = if let Some(referrer_id) = stats.referrer_id {
            let computed_referrer_fee = service_fee
                .checked_div(BASIS_P.into())
                .unwrap_or(0)
                .checked_mul(self.referrer_fee_share as u128)
                .expect("multiplication overflow");

            if computed_referrer_fee > 0 {
                log!(
                    "Affiliate reward for @{} is {}",
                    referrer_id,
                    computed_referrer_fee
                );
                self.internal_update_stats(
                    Some(token_id),
                    &referrer_id,
                    UpdateStatsAction::AddAffiliateReward,
                    None,
                    Some(computed_referrer_fee),
                );
                // transfer fee to referrer
                self.internal_transfer(&token_id, &referrer_id, computed_referrer_fee.into());
            }

            computed_referrer_fee
        } else {
            0
        };

        referrer_fee
    }

    pub(crate) fn internal_tie_refund(
        &mut self,
        game_id: &GameId,
        token_id: &TokenContractId,
        refund_amount: Balance,
    ) {
        let (player1, player2) = self.internal_get_game_players(game_id);
        self.internal_transfer(&token_id, &player1, refund_amount.into());
        self.internal_transfer(&token_id, &player2, refund_amount.into());
    }

    pub(crate) fn internal_stop_expired_game(&mut self, game_id: &GameId, looser: AccountId) {
        let mut game: Game = self.internal_get_game(&game_id);
        assert_eq!(
            game.game_state,
            GameState::Active,
            "Current game isn't active"
        );

        self.internal_update_stats(None, &looser, UpdateStatsAction::AddPenaltyGame, None, None);

        let (player1, player2) = self.internal_get_game_players(game_id);

        let winner = if looser == player1 {
            player2.clone()
        } else if looser == player2 {
            player1.clone()
        } else {
            panic!("Account @{} not in this game. GameId: {} ", looser, game_id)
        };

        let balance = self.internal_distribute_reward(game_id, Some(&winner));
        game.change_state(GameState::Finished);
        self.internal_update_game(game_id, &game);

        let game_to_store = GameLimitedView {
            game_result: views::GameResult::Win(winner),
            player1,
            player2,
            reward_or_tie_refund: GameDeposit {
                token_id: game.reward().token_id,
                balance,
            },
            tiles: game.to_tiles(),
            last_move: None,
        };
        self.internal_store_game(game_id, &game_to_store);
        assert_eq!(
            game.game_state,
            GameState::Finished,
            "Cannot stop. Game in progress"
        );
        self.games.remove(game_id);
    }

    pub(crate) fn is_account_exists(&self, account_id: &AccountId) -> bool {
        if let Some(_stats) = self.stats.get(account_id) {
            true
        } else {
            false
        }
    }

    pub(crate) fn internal_check_player_available(&mut self, account_id: &AccountId) {
        let has_games_started = self
            .games
            .iter()
            .any(|(_game_id, game)| game.contains_player_account_id(account_id));
        assert!(
            !has_games_started,
            "Player @{} already start another game",
            &account_id
        )
    }

    pub(crate) fn internal_add_referrer(&mut self, player_id: &AccountId, referrer_id: &AccountId) {
        if self.stats.get(player_id).is_none() && self.is_account_exists(referrer_id) {
            self.internal_update_stats(
                None,
                player_id,
                UpdateStatsAction::AddReferral,
                Some(referrer_id.clone()),
                None,
            );
            self.internal_update_stats(
                None,
                referrer_id,
                UpdateStatsAction::AddAffiliate,
                Some(player_id.clone()),
                None,
            );
            log!("Referrer {} added for {}", referrer_id, player_id);
        } else {
            log!("Referrer was not added")
        }
    }

    pub(crate) fn internal_get_game(&self, game_id: &GameId) -> Game {
        self.games.get(game_id).expect("Game not found")
    }

    pub(crate) fn internal_update_game(&mut self, game_id: &GameId, game: &Game) {
        self.games.insert(game_id, &game);
    }

    pub(crate) fn internal_get_game_players(&self, game_id: &GameId) -> (AccountId, AccountId) {
        let game = self.internal_get_game(game_id);
        game.get_player_accounts()
    }

    pub(crate) fn internal_get_game_reward(&self, game_id: &GameId) -> GameDeposit {
        let game = self.internal_get_game(game_id);
        game.reward()
    }

    pub(crate) fn get_stored_games_num(&self) -> u8 {
        return self.stored_games.len() as _;
    }

    pub(crate) fn internal_store_game(&mut self, game_id: &GameId, game: &GameLimitedView) {
        let current_games_stored = self.get_stored_games_num();
        if current_games_stored + 1 == self.max_stored_games {
            self.stored_games
                .remove(&(*game_id - current_games_stored as u64));
        }
        self.stored_games.insert(game_id, game);
    }
}
