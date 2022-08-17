use near_sdk::{
    AccountId, Balance, BorshStorageKey, Gas, Duration, PanicOnDefault,
    Promise, PromiseOrValue, PromiseResult, assert_one_yocto
};
use near_sdk::{
    env, ext_contract, log, near_bindgen, ONE_NEAR, ONE_YOCTO, require
};
use near_sdk::json_types::U128;
use near_sdk::borsh::{self, BorshSerialize, BorshDeserialize};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::collections::{UnorderedMap, UnorderedSet};

mod board;
mod callbacks;
mod config;
mod game;
mod game_config;
mod internal;
mod player;
mod stats;
mod token_receiver;
mod views;
mod utils;

use crate::board::*;
use crate::config::*;
use crate::game::*;
use crate::game_config::*;
use crate::player::*;
use crate::stats::*;
use crate::token_receiver::*;
use crate::utils::*;

#[derive(BorshSerialize, BorshStorageKey)]
pub enum StorageKey {
    WhitelistedTokens,
    Games,
    Players,
    /* * */
    Stats,
    Affiliates {account_id : AccountId},
    TotalRewards {account_id : AccountId},
    TotalAffiliateRewards {account_id : AccountId}
}

pub (crate) type MinDeposit = Balance;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    /// Allowed game reward tokens as `TokenContractId` : `MinDeposit`
    whitelisted_tokens: UnorderedMap<TokenContractId, MinDeposit>,
    games: UnorderedMap<GameId, Game>,
    available_players: UnorderedMap<AccountId, GameConfig>,
    /* * */
    stats: UnorderedMap<AccountId, Stats>,
    /// `GameId` which will be set for next created `Game`
    next_game_id: GameId,
    /// service fee percentage in BASIS_P (see `config.rs`)
    service_fee_percentage: u32,
    /// max expected game duration in nanoseconds (see `config.rs`)
    max_game_duration: Duration,
    /// referrer fee percentage from service_fee_percentage in BASIS_P (see `config.rs`)
    referrer_ratio: u32,
    /// debug
    pub reward_computed: Balance    
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(config: Option<Config>) -> Self {
        let (
            service_fee_percentage, 
            max_game_duration,
            referrer_ratio
        ) = if let Some(config) = config {
            config.assert_valid();
            (
                config.service_fee_percentage,
                sec_to_nano(config.max_game_duration_sec),
                config.referrer_ratio
            )
        } else {
            (
                MIN_FEES,
                sec_to_nano(60 * 15),
                BASIS_P / 2
            )
        };
        Self {
            whitelisted_tokens: UnorderedMap::new(StorageKey::WhitelistedTokens),
            games: UnorderedMap::new(StorageKey::Games),
            available_players: UnorderedMap::new(StorageKey::Players),
            stats: UnorderedMap::new(StorageKey::Stats),
            next_game_id: 0,
            service_fee_percentage,
            max_game_duration,
            referrer_ratio,
            reward_computed: 0
        }
    }

    /// Make player available only with NEAR deposits
    #[payable]
    pub fn make_available(
        &mut self,
        game_config: GameConfigNear,
    ) {
        let account_id: &AccountId = &env::predecessor_account_id();
        assert!(self.available_players.get(account_id).is_none(), "Already in the waiting list the list");

        let deposit: Balance = env::attached_deposit();
        assert!(deposit >= MIN_DEPOSIT_NEAR, "Deposit is too small. Attached: {}, Required: {}", deposit, MIN_DEPOSIT_NEAR);

        self.available_players.insert(account_id,
            &GameConfig {
                token_id: AccountId::new_unchecked("near".into()),
                deposit,
                opponent_id: game_config.opponent_id,
                referrer_id: game_config.referrer_id.clone()
            }
        );
        
        self.internal_check_player_available(&account_id);

        if let Some(referrer_id) = game_config.referrer_id {
            self.internal_add_referrer( &account_id, &referrer_id);
        }
    }

    #[payable]
    pub fn make_unavailable(&mut self) {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        match self.available_players.get(&account_id) {
            Some(config) => {
                // refund players deposit
                let token_id = config.token_id.clone();
                self.available_players.remove(&account_id);

                self.internal_transfer(&token_id, &account_id, config.deposit.into())
                    .then(Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .transfer_deposit_callback(account_id, &config)
                );
            },
            None => panic!("You are not available now")
        }
    }

    pub fn start_game(&mut self, player_2_id: AccountId) -> GameId {
        if let Some(player_2_config) = self.available_players.get(&player_2_id) {
            // Check is game initiator (predecessor) player available to play as well
            let player_1_id = env::predecessor_account_id();
            assert_ne!(player_1_id.clone(), player_2_id.clone(), "Find a friend to play");

            // Get predecessor's available deposit
            let player_1_config = self.available_players
                .get(&player_1_id)
                .expect("You are not in available players list!");
            let player_1_config_token = player_1_config.token_id;
            let player_1_deposit = player_1_config.deposit;

            self.internal_check_player_available(&player_1_id);
            
            if let Some(player_id) = player_2_config.opponent_id {
                assert_eq!(player_id, player_1_id, "Wrong account");
            }

            // Deposits from two players must be equal
            assert_eq!(
                player_1_deposit, 
                player_2_config.deposit, 
                "Mismatched deposits for players! You: {}, Opponent {}",
                player_1_deposit,
                player_2_config.deposit
            );

            let game_id = self.next_game_id;
            let token_id = player_2_config.token_id;

            assert_eq!(token_id, player_1_config_token, "Mismatch tokens! Choosen tokens for opponent and you must be the same");
            // deposit * 2
            let balance = match player_2_config.deposit.checked_mul(2) {
                Some(value) => value,
                None => panic!("multiplication overflow, too big deposit amount"),
            };

            let reward = GameDeposit {
                token_id: token_id.clone(),
                balance
            };
            log!("game reward:{} in token {:?} ", balance, token_id.clone());
            
            let seed = near_sdk::env::random_seed();
            let mut game = match seed[0] % 2 {
                0 => {
                    Game::create_game(
                    player_2_id.clone(),
                    player_1_id.clone(),
                    reward
                    )
                },
                _ => {
                    Game::create_game(
                    player_1_id.clone(),
                    player_2_id.clone(),
                    reward
                    )
                },
            };

            game.change_state(GameState::Active);
            self.games.insert(&game_id, &game);

            self.next_game_id += 1;
            self.available_players.remove(&player_1_id);
            self.available_players.remove(&player_2_id);

            if let Some(referrer_id) = player_1_config.referrer_id {
                self.internal_add_referrer(&player_1_id, &referrer_id);
            }
            if let Some(referrer_id) = player_2_config.referrer_id {
                self.internal_add_referrer(&player_2_id, &referrer_id);
            }

            self.internal_update_stats(Some(&token_id), &player_1_id, UpdateStatsAction::AddPlayedGame, None, None);
            self.internal_update_stats(Some(&token_id), &player_2_id, UpdateStatsAction::AddPlayedGame, None, None);
            game_id
        } else {
            panic!("Your opponent is not ready");
        }
    }

    pub fn make_move(&mut self, game_id: &GameId, row: usize, col: usize) -> [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE] {
        let mut game = self.internal_get_game(game_id);
        let init_game_state = game.game_state;

        assert_eq!(env::predecessor_account_id(), game.current_player_account_id(), "No access");
        assert_eq!(init_game_state, GameState::Active, "Current game isn't active");

        match game.board.check_move(row, col) {
            Ok(_) => {
                // fill board tile with current player piece
                game.board.tiles[row][col] = Some(game.current_piece);
                // switch piece to other one
                game.current_piece = game.current_piece.other();
                // switch player
                game.current_player_index = 1 - game.current_player_index;
                game.board.update_winner(row, col);

                if let Some(winner) = game.board.winner {
                    // change game state to Finished
                    game.change_state(GameState::Finished);
                    self.internal_update_game(game_id, &game);
                    // get winner account, if there is Tie - refund to both players
                    // with crop service fee amount from it
                    let winner_account:Option<&AccountId> = match winner {
                        board::Winner::X => game.get_player_acc_by_piece(Piece::X),
                        board::Winner::O => game.get_player_acc_by_piece(Piece::O),
                        board::Winner::Tie => None,
                    };
               
                    let reward = if winner_account.is_some() {
                        // SOME WINNER
                        log!("\nGame over! {} won!", winner_account.unwrap());
                        self.internal_distribute_reward(game_id, winner_account)
                    } else {
                        // TIE
                        log!("\nGame over! Tie!");
                        self.internal_distribute_reward(game_id, None)
                    };
                    self.reward_computed = reward;
                    self.internal_stop_game(game_id);
                    return game.board.tiles;
                };
            },
            Err(e) => match e {
                MoveError::GameAlreadyOver => panic!("Game is already finished"),
                MoveError::InvalidPosition { row, col } => panic!(
                    "Provided position is invalid: row: {} col: {}", row, col),
                MoveError::TileFilled { other_piece, row, col } => panic!(
                    "The tile row: {} col: {} already contained another piece: {:?}", row, col, other_piece
                ),
            },
        }
        if game.game_state == GameState::Active {

            game.total_turns += 1;
            game.last_turn_timestamp = env::block_timestamp();
            game.current_duration += game.last_turn_timestamp - game.initiated_at;

            if game.current_duration <= self.max_game_duration {
                self.internal_update_game(game_id, &game);
                return game.board.tiles;
            } else {
                log!("Game duration expired. Required:{} Current:{} ", self.max_game_duration, game.current_duration);
                self.internal_stop_expired_game(game_id, env::predecessor_account_id());
                return game.board.tiles;
            }
        } else {
            panic!("Something wrong with game id: {} state", game_id)
        }

    }

    #[payable]
    pub fn give_up(&mut self, game_id: &GameId) {
        assert_one_yocto();
        let mut game: Game = self.internal_get_game(&game_id);
        assert_eq!(game.game_state, GameState::Active, "Current game isn't active");
        
        let account_id = env::predecessor_account_id();

        let (player1, player2) = self.internal_get_game_players(game_id);
        
        let winner = if account_id == player1{
            player2
        } else if account_id == player2 {
            player1
        } else {
            panic!("You are not in this game. GameId: {} ", game_id)
        };

        self.internal_distribute_reward(game_id, Some(&winner));
        game.change_state(GameState::Finished);
        self.internal_update_game(game_id, &game);
        self.internal_stop_game(game_id);
    }

    pub fn stop_game(&mut self, game_id: &GameId) {
        let mut game: Game = self.internal_get_game(&game_id);
        assert_eq!(game.game_state, GameState::Active, "Current game isn't active");

        let account_id = env::predecessor_account_id();
        assert_ne!(env::predecessor_account_id(), game.current_player_account_id(), "No access");

        let (player1, player2) = self.internal_get_game_players(game_id);

        game.current_duration = env::block_timestamp() - game.initiated_at;
        assert!(game.current_duration >= self.max_game_duration, "Too early to stop the game");

        let (winner, looser) = if account_id == player1 {
            (player1, player2)
        } else if account_id == player2 {
            (player2, player1)
        } else {
            panic!("You are not in this game. GameId: {} ", game_id)
        };

        self.internal_update_stats(
            Some(&game.reward().token_id), 
            &looser, 
            UpdateStatsAction::AddPenaltyGame, 
            None, 
            None);

        self.internal_distribute_reward(game_id, Some(&winner));
        game.change_state(GameState::Finished);
        self.internal_update_game(game_id, &game);
        self.internal_stop_game(game_id);
    }

    pub (crate) fn internal_update_game(&mut self, game_id: &GameId, game: &Game) {
        self.games.insert(game_id, &game);
    }

    /// Views ///

    pub fn get_whitelisted_tokens(&self) -> Vec<(TokenContractId, MinDeposit)> {
        self.whitelisted_tokens.to_vec()
    }
    pub fn get_token_min_deposit(&self, token_id: &TokenContractId) -> U128 {
        self.whitelisted_tokens
            .get(token_id)
            .expect("Token isn't whitelisted")
            .into()
    }
    pub fn get_available_players(&self) -> Vec<(AccountId, GameConfig)> {
        self.available_players.to_vec()
    }
    pub fn get_active_games(&self) -> Vec<(GameId, Game)> {
        self.games.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, Balance};
    use super::*;

    const ONE_CHEDDAR:Balance = ONE_NEAR;

    fn user() -> AccountId {
        "user".parse().unwrap()
    }
    fn opponent() -> AccountId {
        "opponent.near".parse().unwrap()
    }
    fn referrer() -> AccountId {
        "referrer.near".parse().unwrap()
    }
    fn acc_cheddar() -> AccountId {
        "cheddar".parse().unwrap()
    }
    fn near() -> AccountId {
        "near".parse().unwrap()
    }

    fn setup_contract(
        predecessor: AccountId,
        service_fee_percentage: Option<u32>,
        referrer_fee: Option<u32>,
        max_game_duration_sec: Option<u32>
    ) -> (VMContextBuilder, Contract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context.build());
        let config = if service_fee_percentage.is_none() && max_game_duration_sec.is_none() && referrer_fee.is_none(){
            None
        } else {
            Some(Config {
                service_fee_percentage: service_fee_percentage.unwrap(),
                referrer_ratio: referrer_fee.unwrap_or(BASIS_P / 2),
                max_game_duration_sec: max_game_duration_sec.unwrap(),
            })
        };

        let contract = Contract::new(
            config
        );
        testing_env!(context
            .predecessor_account_id(predecessor.clone())
            .signer_account_id(predecessor.clone())
            .build());
        (context, contract)
    }

    fn whitelist_token(
        ctr: &mut Contract,
    ) {
        ctr.whitelist_token(acc_cheddar().clone(), U128(ONE_CHEDDAR / 10))
    }

    fn make_available_near(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        amount: Balance,
        opponent_id: Option<AccountId>, 
        referrer_id: Option<AccountId> 
    ) {
        testing_env!(ctx
            .attached_deposit(amount)
            .predecessor_account_id(user.clone())
            .signer_account_id(user.clone())
            .build());
        ctr.make_available(GameConfigNear { 
            deposit: amount, 
            opponent_id, 
            referrer_id 
        });
    }

    fn make_available_ft(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        amount: Balance,
        msg: String
    ) {
        testing_env!(ctx
            .attached_deposit(ONE_YOCTO)
            .predecessor_account_id(acc_cheddar().clone())
            .signer_account_id(user.clone())
            .build());
        ctr.ft_on_transfer(user.clone(), U128(amount), msg);
    }

    fn make_unavailable(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
    ) {
        testing_env!(ctx
            .attached_deposit(ONE_YOCTO)
            .predecessor_account_id(user.clone())
            .signer_account_id(user.clone())
            .build());
        ctr.make_unavailable();
    }

    fn start_game(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        opponent: &AccountId,
    ) -> GameId {
        testing_env!(ctx
            .predecessor_account_id(user.clone())
            .build());
        ctr.start_game(opponent.clone())
    }

    fn make_move(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        game_id: &GameId,
        row: usize,
        col: usize
    ) -> [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE] {
        testing_env!(ctx
            .predecessor_account_id(user.clone())
            .build());
        ctr.make_move(game_id, row, col)
    }

    fn stop_game(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        game_id: &GameId,
        forward_time_sec: u32
    ) {
        let nanos = sec_to_nano(forward_time_sec);
        testing_env!(ctx
            .predecessor_account_id(user.clone())
            .attached_deposit(ONE_YOCTO)
            .block_timestamp(nanos)
            .build());
        ctr.stop_game(&game_id)
    }

    fn fast_forward(
        ctx: &mut VMContextBuilder,
        time: u32
    ) {
        let nanos = sec_to_nano(time);
        testing_env!(ctx.block_timestamp(nanos).build());
    }

    fn get_board_current_player(game: &Game) -> AccountId {
        game.current_player_account_id()
    }

    /// This function is used to print out the board in a human readable way
    fn print_tiles(tiles: &[[Option<Piece>; BOARD_SIZE]; BOARD_SIZE]) {
        // The result of this function will be something like the following:
        //   A B C
        // 1 x ▢ ▢
        // 2 ▢ ▢ o
        // 3 ▢ ▢ ▢
        print!("  ");
        for j in 0..tiles[0].len() as u8 {
            // `b'A'` produces the ASCII character code for the letter A (i.e. 65)
            print!(" {}", (b'A' + j) as char);
        }
        // This prints the final newline after the row of column letters
        println!();
        for (i, row) in tiles.iter().enumerate() {
            // We print the row number with a space in front of it
            print!(" {}", i + 1);
            for tile in row {
                print!(" {}", match *tile {
                    Some(Piece::X) => "x",
                    Some(Piece::O) => "o",
                    None => "\u{25A2}", // empty tile pretty print "▢"
                });
            }
            println!();
        }
        println!();
    }

    #[test]
    fn test_whitelist_token() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
    }
    #[test]
    fn make_available_unavailable_near() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        assert!(ctr.get_available_players().is_empty());
        make_available_near(&mut ctx, &mut ctr, &user(), ONE_NEAR, None, Some(referrer()));
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_NEAR, Some(user()), None);
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfig)>::from([
            (user(), GameConfig { 
                token_id: near(), 
                deposit: ONE_NEAR, 
                opponent_id: None, 
                referrer_id: Some(referrer()) 
            }),
            (opponent(), GameConfig { 
                token_id: near(), 
                deposit: ONE_NEAR, 
                opponent_id: Some(user()), 
                referrer_id: None 
            }),
        ]));
        make_unavailable(&mut ctx, &mut ctr, &user());
        make_unavailable(&mut ctx, &mut ctr, &opponent());
        assert!(ctr.get_available_players().is_empty());
    }
    #[test]
    fn test_make_available_unavailable() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: Some(referrer()) 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfig)>::from([
            (user(), GameConfig { 
                token_id: acc_cheddar(), 
                deposit: ONE_CHEDDAR, 
                opponent_id: Some(opponent()), 
                referrer_id: Some(referrer()) 
            }),
            (opponent(), GameConfig { 
                token_id: acc_cheddar(), 
                deposit: ONE_CHEDDAR, 
                opponent_id: Some(user()), 
                referrer_id: None 
            }),
        ]));
        make_unavailable(&mut ctx, &mut ctr, &user());
        make_unavailable(&mut ctx, &mut ctr, &opponent());
        assert!(ctr.get_available_players().is_empty());
    }
    #[test]
    #[should_panic(expected="Mismatch tokens! Choosen tokens for opponent and you must be the same")]
    fn start_game_diff_tokens() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: Some(referrer()) 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");

        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, None, None);
        start_game(&mut ctx, &mut ctr, &user(), &opponent());
    }
    #[test]
    fn test_give_up() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: Some(referrer()) 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfig)>::from([
            (user(), GameConfig { 
                token_id: acc_cheddar(), 
                deposit: ONE_CHEDDAR, 
                opponent_id: Some(opponent()), 
                referrer_id: Some(referrer()) 
            }),
            (opponent(), GameConfig { 
                token_id: acc_cheddar(), 
                deposit: ONE_CHEDDAR, 
                opponent_id: Some(user()), 
                referrer_id: None 
            }),
        ]));
        testing_env!(ctx
            .attached_deposit(ONE_YOCTO)
            .predecessor_account_id(user().clone())
            .build());
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        ctr.give_up(&game_id);
        let player_1_stats = ctr.get_stats(&user());
        let player_2_stats = ctr.get_stats(&opponent());

        assert!(
            player_1_stats.games_played == player_2_stats.games_played
        );
        assert!(
            player_1_stats.victories_num == 0 && player_2_stats.victories_num == 1
        );
        assert_eq!(
            player_2_stats.total_reward, Vec::from([(acc_cheddar(), (2 * ONE_CHEDDAR - ((2 * ONE_CHEDDAR / BASIS_P as u128) * MIN_FEES as u128)))]) 
        );
        assert!(player_1_stats.total_reward.is_empty());
    }
    #[test]
    fn test_game_basics() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: Some(referrer()) 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);

        let player_1_stats = ctr.get_stats(&opponent());
        let player_2_stats = ctr.get_stats(&user());
        println!("{:#?}", player_1_stats);
        println!("{:#?}", player_2_stats);
        assert!(
            player_1_stats.games_played == player_2_stats.games_played
        );
        assert!(
            player_2_stats.victories_num == 0 && player_1_stats.victories_num == 1
        );   
        assert_eq!(
            player_1_stats.total_reward.clone(), Vec::from([
                (acc_cheddar(), (2 * ONE_CHEDDAR - ((2 * ONE_CHEDDAR / BASIS_P as u128 )* MIN_FEES as u128)))
            ])
        );
        assert!(player_2_stats.total_reward.is_empty());
    }

    #[test]
    fn test_game_basics_near() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        assert!(ctr.get_available_players().is_empty());
        make_available_near(&mut ctx, &mut ctr, &user(), ONE_NEAR, None, None);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_NEAR, None, None);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);

        let player_1_stats = ctr.get_stats(&opponent());
        let player_2_stats = ctr.get_stats(&user());
        println!("{:#?}", player_1_stats);
        println!("{:#?}", player_2_stats);
        assert!(
            player_1_stats.games_played == player_2_stats.games_played
        );
        assert!(
            player_2_stats.victories_num == 0 && player_1_stats.victories_num == 1
        );   
        assert_eq!(
            player_1_stats.total_reward.clone(), Vec::from([
                (
                    "near".parse().unwrap(), 
                    (2 * ONE_NEAR - ((2 * ONE_NEAR / BASIS_P as u128 )* MIN_FEES as u128))
                )
            ])
        );
        assert!(player_2_stats.total_reward.is_empty());
    }

    #[test]
    fn test_tie_scenario() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: None 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        println!("( {} , {} )", player_1, player_2);

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 1);
        print_tiles(&tiles);

        let player_1_stats = ctr.get_stats(&opponent());
        let player_2_stats = ctr.get_stats(&user());

        assert!(
            player_1_stats.games_played == player_2_stats.games_played
        );
        assert_eq!(
            player_1_stats.victories_num, player_2_stats.victories_num
        );
        assert!(
            player_1_stats.total_reward == player_2_stats.total_reward
        ); 
        assert_eq!(
            ctr.reward_computed,
            (2 * ONE_CHEDDAR - (2 * ONE_CHEDDAR / BASIS_P as u128 * MIN_FEES as u128)) / 2 
        )
    }
    #[test]
    #[should_panic(expected="Too early to stop the game")]
    fn test_stop_game_too_early() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: None 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        println!("( {} , {} )", player_1, player_2);

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
        print_tiles(&tiles);
        
        stop_game(&mut ctx, &mut ctr, &player_2, &game_id, 30);
    }

    #[test]
    #[should_panic(expected="No access")]
    fn test_stop_game_wrong_access() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: None 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        println!("( {} , {} )", player_1, player_2);

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
        print_tiles(&tiles);
        
        stop_game(&mut ctx, &mut ctr, &player_1, &game_id, 601);
    }

    #[test]
    fn test_expired_game() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: None 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        println!("( {} , {} )", player_1, player_2);

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        print_tiles(&tiles);
        testing_env!(ctx
            .predecessor_account_id(player_2.clone())
            .block_timestamp(sec_to_nano(601))
            .build()
        );
        // player2 turn too slow
        ctr.make_move(&game_id, 1, 2);
        assert!(ctr.get_stats(&player_1).victories_num == 1);
        assert!(ctr.get_stats(&player_2).victories_num == 0);
        assert_eq!(
            ctr.get_stats(&player_1).total_reward,
            Vec::from([
                (
                    acc_cheddar(),
                    (2 * ONE_CHEDDAR - (2 * ONE_CHEDDAR / BASIS_P as u128 * MIN_FEES as u128)) 
                )
            ])
        )
    }

    #[test]
    fn test_stop_game() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(60 * 10));
        whitelist_token(&mut ctr);
        assert_eq!(ctr.get_whitelisted_tokens(), Vec::from([
            (acc_cheddar(), ONE_CHEDDAR / 10)
        ]));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: None 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
        let gc2 = GameConfigArgs { 
            opponent_id: Some(user()), 
            referrer_id: None 
        };
        let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
        let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
        let game = ctr.internal_get_game(&game_id);
        let player_1 = game.current_player_account_id().clone();
        let player_2 = game.next_player_account_id().clone();

        println!("( {} , {} )", player_1, player_2);

        assert_ne!(player_1, game.next_player_account_id());
        assert_ne!(game.players[0].piece, game.players[1].piece);
        assert_eq!(player_1, game.players[0].account_id);
        assert_eq!(player_2, game.players[1].account_id);
        assert_eq!(game.board.current_piece, game.players[0].piece);

        assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

        let mut tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        print_tiles(&tiles);
        tiles = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
        print_tiles(&tiles);
        
        stop_game(&mut ctx, &mut ctr, &player_2, &game_id, 601);
        
        let player_1_stats = ctr.get_stats(&opponent());
        let player_2_stats = ctr.get_stats(&user());

        assert!(
            player_1_stats.games_played == player_2_stats.games_played
        );
        assert!(
            player_2_stats.victories_num == 1 && player_1_stats.victories_num == 0
        );
        assert!(
            !player_2_stats.total_reward.is_empty() && player_1_stats.total_reward.is_empty()
        ); 
        assert_eq!(
            player_2_stats.total_reward,
            Vec::from([
                (
                    acc_cheddar(),
                    (2 * ONE_CHEDDAR - (2 * ONE_CHEDDAR / BASIS_P as u128 * MIN_FEES as u128)) 
                )
            ])
        )
    }
}