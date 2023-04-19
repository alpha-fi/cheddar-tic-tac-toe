use std::fs::create_dir;

use near_contract_standards::storage_management::StorageBalance;
use near_sdk::{
    AccountId, Balance, BorshStorageKey, Gas, Duration, PanicOnDefault,
    Promise, PromiseOrValue, PromiseResult, assert_one_yocto, Timestamp
};
use near_sdk::{
    env, ext_contract, log, near_bindgen, ONE_YOCTO, require
};
use near_sdk::json_types::{U128};
use near_sdk::borsh::{self, BorshSerialize, BorshDeserialize};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::collections::{UnorderedMap, UnorderedSet};
use stats::UserPenalties;
use views::{GameLimitedView};

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

use crate::config::*;
use crate::game::*;
use crate::game_config::*;
use crate::player::*;
use crate::stats::*;
use crate::token_receiver::*;
use crate::utils::*;
use crate::views::GameResult;

#[derive(BorshSerialize, BorshStorageKey)]
pub enum StorageKey {
    Games,
    StoredGames,
    GameBoard {game_id: GameId},
    Players,
    /* * */
    Stats,
    Affiliates {account_id : AccountId},
    TotalRewards {account_id : AccountId},
    TotalAffiliateRewards {account_id : AccountId},
    RegisteredPlayers,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Vault { 
    total_rewards: Balance,
    storage_deposit: Balance,
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    cheddar: AccountId,
    min_deposit: Balance,
    games: UnorderedMap<GameId, Game>,
    available_players: UnorderedMap<AccountId, GameConfig>,
    /* * */
    stats: UnorderedMap<AccountId, Stats>,
    /// `GameId` which will be set for next created `Game`
    next_game_id: GameId,
    /// service fee percentage in BASIS_P (see `config.rs`)
    service_fee: u16,
    /// max expected game duration in seconds (see `config.rs`)
    max_game_duration: u64,
    /// referrer fee percentage from service_fee_percentage in BASIS_P (see `config.rs`)
    referrer_fee_share: u16,
    /// system updates
    pub last_update_timestamp: Timestamp,
    /// max expected turn duration in seconds (max_game_duration / max possible turns num)
    max_turn_duration: Duration,
    /// storage for printing results
    pub max_stored_games: u8,
    pub stored_games: UnorderedMap<GameId, GameLimitedView>,
    /// registered players and their total_rewards and deposit NEAR to cover storage
    pub registered_players: UnorderedMap<AccountId, Vault>,
}
#[near_bindgen]
impl Contract {
    #[init]
    /// @cheddar: the cheddart token account address
    pub fn new(cheddar: AccountId, min_deposit: Balance, config: Option<Config>) -> Self {
        let config = config.unwrap_or(Config {
            fee: MAX_FEES,
            referrer_fee_share: 500, // 5%
            max_game_duration: MAX_GAME_DURATION,
            max_stored_games:    50
        });
        let min_min_deposit = MIN_DEPOSIT_CHEDDAR;
        assert!(min_deposit >= min_min_deposit, "min_deposit must be at least {}", min_min_deposit);
        Self {
            cheddar,
            min_deposit,
            games: UnorderedMap::new(StorageKey::Games),
            available_players: UnorderedMap::new(StorageKey::Players),
            stats: UnorderedMap::new(StorageKey::Stats),
            next_game_id: 0,
            service_fee: config.fee,
            max_game_duration: config.max_game_duration,
            referrer_fee_share: config.referrer_fee_share,
            last_update_timestamp: 0,
            max_turn_duration: 2*60,
            max_stored_games: config.max_stored_games,
            stored_games: UnorderedMap::new(StorageKey::StoredGames),
            registered_players: UnorderedMap::new(StorageKey::RegisteredPlayers),
        }
    }

    /// Make player available only with CHEDDAR deposits
    #[payable]
    pub fn make_available(
        &mut self,
        game_config: Option<GameConfigNear>,
        bet: Balance,
        available_for: Duration,
    ) {
        let cur_timestamp = env::block_timestamp();
        // checkpoint
        self.internal_ping_expired_players(cur_timestamp);

        let account_id: &AccountId = &env::predecessor_account_id();
        assert!(self.available_players.get(account_id).is_none(), "Already in the waiting list the list");

        let deposit: Balance = bet;
        assert!(bet >= MIN_BET_CHEDDAR, "Bet is too small. Required at least: {}", MIN_DEPOSIT_CHEDDAR);

        let (opponent_id, referrer_id) = if let Some(game_config) = game_config {
            (game_config.opponent_id, game_config.referrer_id.clone())
        } else {
            (None, None)
        };
        self.available_players.insert(account_id,
            &GameConfig {
                deposit,
                opponent_id,
                referrer_id: referrer_id.clone(),
                created_at: nano_to_sec(cur_timestamp),
                available_to: nano_to_sec(cur_timestamp) + available_for,
            }
        );
        
        self.internal_check_player_available(&account_id);

        if let Some(referrer_id) = referrer_id {
            self.internal_add_referrer( &account_id, &referrer_id);
        }
    }

    pub fn get_registered_player(&self, account_id: &AccountId) -> Vault {
        return self.registered_players.get(account_id).expect("Player not registered");
    }

    pub fn make_unavailable(&mut self) {
        let account_id = env::predecessor_account_id();
        self.refund_player(&account_id);
    }
    pub fn refund_player(&mut self, account_id: &AccountId) {
        match self.available_players.get(&account_id) {
            Some(config) => {
                let bet = config.deposit;
                self.available_players.remove(&account_id);
                let mut vault = self.get_registered_player(&account_id);
                vault.total_rewards += bet;
                self.registered_players.insert(&account_id, &vault);
            },
            None => () // skip
        }
    }
    pub fn is_user_registered(&self, account_id: &AccountId) -> bool {
        let key = self.registered_players.get(account_id);
        match key {
            None => false,
            Some(_) => true,
        }
    }
    #[payable]
    pub fn withdraw_cheddar(&mut self, amount: Balance) {
        assert_one_yocto();
        let caller_id = env::predecessor_account_id();
        let mut vault = self.registered_players.get(&caller_id).expect("User is not registered. Cannot withdraw.");
        if amount <= vault.total_rewards {
            ext_ft::ext(self.cheddar.clone())
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .with_attached_deposit(ONE_YOCTO)
            .ft_transfer(caller_id.clone(), amount.into(), None).then(
                Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .cheddar_withdraw_callback(&caller_id.clone(), vault.clone()),
            );
            vault.storage_deposit -= amount;
            self.registered_players.insert(&caller_id, &vault);

        } else {
            panic!("Insufficient balance. Requested {}, available {}.", amount, vault.total_rewards);
        }
    }
    pub fn deposit_cheddar(&mut self, account_id: &AccountId, amount: Balance) {
        let mut vault = self.registered_players.get(account_id).unwrap();
        vault.storage_deposit += amount;
        self.registered_players.insert(account_id, &vault);
    }
    pub fn register_player(&mut self, account_id: &AccountId){
        let valut = Vault {total_rewards: 0, storage_deposit: STORAGE_COST_PER_USER};
        self.registered_players.insert(account_id, &valut);
    }

    #[allow(unused_variables)]
    #[payable]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let storage_deposit: Balance = env::attached_deposit();
        let account_id = account_id
            .map(|a| a.into())
            .unwrap_or_else(|| env::predecessor_account_id());
        if self.is_user_registered(&account_id) {
            log!("The account is already registered, refunding the deposit");
            if storage_deposit > 0 {
                Promise::new(env::predecessor_account_id()).transfer(storage_deposit);
            }
        } else {
            assert!(
                storage_deposit >= STORAGE_COST_PER_USER,
                "The attached deposit is less than the minimum storage balance ({})",
                STORAGE_COST_PER_USER
            );
            self.register_player(&account_id);

            let refund = storage_deposit - STORAGE_COST_PER_USER;
            if refund > 0 {
                Promise::new(env::predecessor_account_id()).transfer(refund);
            }
        }
        self.storage_balance()
    }
    pub fn storage_balance(&self) -> StorageBalance {
        StorageBalance {
            total: STORAGE_COST_PER_USER.into(),
            available: U128::from(0),
        }
    }
    /// Method not supported. Unregister the account using 'unregister_account()' method. It unregisters the account and withdraws deposited NEAR.
    #[allow(unused_variables)]
    pub fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        panic!("Storage withdraw not possible, unregister the account instead");
    }
    pub fn unregister_account(&mut self) -> StorageBalance {
        let account_id = env::predecessor_account_id();
        if !self.is_user_registered(&account_id) {
            log!("The account is not registered, cannot close an unregistered account");
        } else {
            let refund = self.registered_players.remove(&account_id).unwrap().storage_deposit;
            if refund > 0 {
                Promise::new(env::predecessor_account_id()).transfer(refund);
            }
        }
        self.storage_balance()
    }
    pub fn start_game(&mut self, player_2_id: AccountId) -> GameId {
        if let Some(player_2_config) = self.available_players.get(&player_2_id) {
            // Check is game initiator (predecessor) player available to play as well
            let player_1_id = env::predecessor_account_id();
            assert_ne!(player_1_id.clone(), player_2_id.clone(), "you can't play with yourself");

            // Get predecessor's available deposit
            let player_1_config = self.internal_get_available_player(&player_1_id);
            let player_1_deposit = player_1_config.deposit;

            self.internal_check_player_available(&player_1_id);

            // we can't play in parallel with someone else?
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
            // deposit * 2
            let balance = match player_2_config.deposit.checked_mul(2) {
                Some(value) => value,
                None => panic!("multiplication overflow, too big deposit amount"),
            };

            let reward = GameDeposit {
                balance: balance.into()
            };
            log!("game reward:{} in token {:?} ", balance, self.cheddar);
            
            let seed = near_sdk::env::random_seed();
            let (first_player, second_player) = match seed[0] % 2 {
                0 => (player_2_id.clone(), player_1_id.clone()),
                _ => (player_1_id.clone(), player_2_id.clone())
            };
            let mut game = Game::create_game(game_id, first_player, second_player, reward);
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

            self.internal_update_stats(&player_1_id, UpdateStatsAction::AddPlayedGame, None, None);
            self.internal_update_stats(&player_2_id, UpdateStatsAction::AddPlayedGame, None, None);
            game_id
        } else {
            panic!("Your opponent is not ready");
        }
    }
    /// returns (coords, piece, game_result, last_turn_timestamp)
    pub fn get_last_move(&self, game_id: &GameId) -> (Option<Coords>, Piece, Option<GameResult>, Option<Timestamp>){
        let stored_game = self.stored_games.get(game_id);
        if let Some(game) = stored_game {
            if game.last_move.is_none() {
                return (None, Piece::X, Some(game.game_result), None);
            }
            let (coords, piece) = game.last_move.unwrap();
            return (Some(coords), piece, Some(game.game_result), None);
        } else {
        let game = self.internal_get_game(game_id);
        let game_result = game.get_winner();
        return (game.get_last_move(), game.current_piece.other(), game_result, Some(game.last_turn_timestamp));
        }
    }

    pub fn make_move(&mut self, game_id: &GameId, coords: Coords) -> Option<GameResult> {
        let cur_timestamp: Duration = nano_to_sec(env::block_timestamp()) as Duration;
        //checkpoint
        self.internal_ping_expired_games(cur_timestamp);

        let mut game = self.internal_get_game(game_id);

        assert_eq!(env::predecessor_account_id(), game.current_player_account_id(), "not your turn");
        assert_eq!(game.game_state, GameState::Active, "Current game isn't active");
        match game.check_move(&coords) {
            Ok(_) => {
                // fill board tile with current player piece
                game.board.insert(&coords, &game.current_piece);
                // set the last move 
                game.last_move = Some(coords.clone());
                // switch piece to other one
                game.current_piece = game.current_piece.other();
                game.current_piece = game.current_piece;
                // switch player
                game.current_player_index = 1 - game.current_player_index;
                game.update_winner(&coords);
                if let Some(winner) = game.winner.clone() {
                    // change game state to Finished
                    game.change_state(GameState::Finished);
                    self.internal_update_game(game_id, &game);
                    // get winner account, if there is Tie - refund to both players
                    // with crop service fee amount from it
                    let winner_account:Option<&AccountId> = match winner {
                        game::Winner::X => game.get_player_acc_by_piece(Piece::X),
                        game::Winner::O => game.get_player_acc_by_piece(Piece::O),
                        game::Winner::Tie => None,
                    };
               
                    let balance = if winner_account.is_some() {
                        // SOME WINNER
                        log!("\nGame over! {} won!", winner_account.unwrap());
                        self.internal_distribute_reward(game_id, winner_account)
                    } else {
                        // TIE
                        log!("\nGame over! Tie!");
                        self.internal_distribute_reward(game_id, None)
                    };

                    let game_result = match winner_account {
                        Some(winner) => GameResult::Win(winner.clone()),
                        None => GameResult::Tie,
                    };

                    let (player1, player2) = game.get_player_accounts();

                    let game_to_store = GameLimitedView{
                        game_result,
                        player1,
                        player2,
                        reward_or_tie_refund: GameDeposit {
                            balance
                        },
                        tiles: game.to_tiles(),
                        last_move: Some((coords, game.current_piece.other())),
                    };

                    self.internal_store_game(game_id, &game_to_store);
                    assert_eq!(
                        game.game_state,
                        GameState::Finished,
                        "Cannot stop. Game in progress"
                    );
                    let game_result = game.get_winner();

                    self.games.remove(game_id);
                    
                    return game_result;
                };
            },
            Err(e) => match e {
                MoveError::GameOver => panic!("Game is finished"),
                MoveError::InvalidPosition { row, col } => panic!(
                    "Provided position is invalid: row: {} col: {}", row, col),
                MoveError::TileFilled { other_piece, row, col } => panic!(
                    "The tile row: {} col: {} already contained another piece: {:?}", row, col, other_piece
                ),
            },
        }
        if game.game_state == GameState::Active {

            game.total_turns += 1;
            // previous turn timestamp
            let previous_turn_timestamp = game.last_turn_timestamp;
            // this turn timestamp
            game.last_turn_timestamp = cur_timestamp;
            // this game duration 
            game.duration = cur_timestamp - game.initiated_at;

            if previous_turn_timestamp == 0 {
                if cur_timestamp - game.initiated_at > self.max_turn_duration {
                    log!("Turn duration expired. Required:{} Current:{} ", self.max_turn_duration, cur_timestamp - game.initiated_at);
                    // looser - current player
                    self.internal_stop_expired_game(game_id, env::predecessor_account_id());
                    return game.get_winner();
                } else {
                    self.internal_update_game(game_id, &game);
                    return game.get_winner();
                }
            }

            // expired turn time scenario - too long movement from current player
            if game.last_turn_timestamp - previous_turn_timestamp > self.max_turn_duration {
                log!("Turn duration expired. Required:{} Current:{} ", self.max_turn_duration, game.last_turn_timestamp - previous_turn_timestamp);
                // looser - current player
                self.internal_stop_expired_game(game_id, env::predecessor_account_id());
                return game.get_winner();
            };

            if game.duration <= self.max_game_duration {
                self.internal_update_game(game_id, &game);
                return game.get_winner();
            } else {
                log!("Game duration expired. Required:{} Current:{} ", self.max_game_duration, game.duration);
                // looser - current player
                self.internal_stop_expired_game(game_id, env::predecessor_account_id());
                return game.get_winner();
            }
        } else {
            panic!("Something wrong with game id: {} state", game_id)
        }

    }

    #[payable]
    pub fn give_up(&mut self, game_id: &GameId) -> Option<GameResult>{
        assert_one_yocto();
        let mut game: Game = self.internal_get_game(&game_id);
        assert_eq!(game.game_state, GameState::Active, "Current game isn't active");
        
        let account_id = env::predecessor_account_id();

        let (player1, player2) = self.internal_get_game_players(game_id);
        
        let winner = if account_id == player1{
            player2.clone()
        } else if account_id == player2 {
            player1.clone()
        } else {
            panic!("You are not in this game. GameId: {} ", game_id)
        };

        let balance = self.internal_distribute_reward(game_id, Some(&winner));
        game.change_state(GameState::Finished);
        self.internal_update_game(game_id, &game);
        assert_eq!(
            game.game_state,
            GameState::Finished,
            "Cannot stop. Game in progress"
        );

        return self.store_game(game_id, &winner, &game.get_opponent(&winner), balance);
    }

    pub fn claim_timeout_win(&mut self, game_id: &GameId) -> Option<GameResult> {
        let game: Game = self.internal_get_game(&game_id);
        let player = env::predecessor_account_id();
        if game.claim_timeout_win(&player) == false {
            log!("can't claim the win, timeout didn't pass");
            return None;
        }
        let looser = game.get_opponent(&player);
        let balance = self.internal_distribute_reward(game_id, Some(&player));

        return self.store_game(game_id, &player, &looser, balance)
    }

    pub fn store_game(&mut self, game_id: &GameId, winner: &AccountId, looser: &AccountId, balance: U128) -> Option<GameResult> {
        let game: Game = self.internal_get_game(&game_id);
        let last_move = game.last_move.clone().map(|coords|  {
          let piece = game.board.get(&coords).unwrap();
          return (coords, piece);
        });

        self.games.remove(game_id);
        let game_to_store = GameLimitedView{
            game_result: GameResult::Win(winner.clone()),
            player1: winner.clone(),
            player2: looser.clone(),
            reward_or_tie_refund: GameDeposit {
                balance
            },
            tiles: game.to_tiles(),
            last_move: last_move
        };
        self.internal_store_game(game_id, &game_to_store);
        return Some(game_to_store.game_result);
    }
    
    #[private]
    pub fn update_fee(&mut self,  fee: u16) {
        assert!(fee <= MAX_FEES, "fees must be in range 0..500 which corresponds to 0..5%");
        self.service_fee = fee;
    }
}

#[cfg(test)]
mod tests {
    use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, Balance, ONE_NEAR};
    use crate::views::{GameView, Tiles};

    use super::*;
    const MIN_GAME_DURATION: u32 = 25 * 60;
    const ONE_CHEDDAR:Balance = ONE_NEAR;
    const MIN_FEES: u32 = 0;

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
                fee: service_fee_percentage.unwrap() as u16,
                referrer_fee_share: referrer_fee.unwrap_or((BASIS_P / 2) as u32) as u16,
                max_game_duration: max_game_duration_sec.unwrap() as u64,
                max_stored_games: 50u8
            })
        };

        let contract = Contract::new(
            acc_cheddar(),
            MIN_DEPOSIT_CHEDDAR,
            config
        );
        testing_env!(context
            .predecessor_account_id(predecessor.clone())
            .signer_account_id(predecessor.clone())
            .build());
        (context, contract)
    }

    fn make_available_near(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        amount: Balance,
        opponent_id: Option<AccountId>, 
        referrer_id: Option<AccountId> ,
        available_for: Duration
    ) {
        testing_env!(ctx
            .attached_deposit(amount)
            .predecessor_account_id(user.clone())
            .signer_account_id(user.clone())
            .build());
        ctr.make_available(Some(GameConfigNear { 
            opponent_id, 
            referrer_id,
        }), amount, available_for);
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
        row: u8,
        col: u8
    ) -> Option<GameResult> {
        testing_env!(ctx
            .predecessor_account_id(user.clone())
            .build());
        ctr.make_move(game_id, Coords{y: row, x: col})
    }
    fn get_last_move(
        ctx: &mut VMContextBuilder,
        ctr: &mut Contract,
        user: &AccountId,
        game_id: &GameId,
    ) -> (Option<Coords>, Piece) {
        testing_env!(ctx
            .predecessor_account_id(user.clone())
            .build());
        let (coords, piece ,_, _) = ctr.get_last_move(game_id);
        return (coords, piece);
    }

    /// This function is used to print out the board in a human readable way
    fn print_tiles(tiles: &Tiles) {
        // The result of this function will be something like the following:
        //   A B C
        // 1 x ▢ ▢
        // 2 ▢ ▢ o
        // 3 ▢ ▢ ▢
        let mut matrix: [[Option<Piece>; BOARD_SIZE as usize]; BOARD_SIZE as usize] =
            Default::default();
        for tile in tiles.x_coords.iter() {
            matrix[tile.y as usize][tile.x as usize] = Some(Piece::X);
        }
        for tile in tiles.o_coords.iter() {
            matrix[tile.y as usize][tile.x as usize] = Some(Piece::O);
        }
        print!("  ");
        for j in 0..matrix[0].len() as u8 {
            // `b'A'` produces the ASCII character code for the letter A (i.e. 65)
            print!(" {}", (b'A' + j) as char);
        }
        // This prints the final newline after the row of column letters
        println!();
        for (i, row) in matrix.iter().enumerate() {
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

    fn game_basics() -> Result<(VMContextBuilder, Contract), std::io::Error> {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION)); // HERE
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
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfigView)>::from([
            (user(), GameConfigView { 
                deposit: U128(ONE_CHEDDAR), 
                opponent_id: Some(opponent()), 
                referrer_id: Some(referrer()),
                created_at: 0
            }),
            (opponent(), GameConfigView {  
                deposit: U128(ONE_CHEDDAR), 
                opponent_id: Some(user()), 
                referrer_id: None,
                created_at: 0
            }),
        ]));

        let user2 = "user2".parse().unwrap();
        let opponent2 = "opponent2".parse().unwrap();

        make_available_near(&mut ctx, &mut ctr, &user2, ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &opponent2, ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);

        let game_id_cheddar = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        let game_id_near = start_game(&mut ctx, &mut ctr, &user2, &opponent2);
        
        let game_cheddar = ctr.internal_get_game(&game_id_cheddar);
        let game_near = ctr.internal_get_game(&game_id_near);
        // cheddar
        let _player_1_c = game_cheddar.current_player_account_id().clone();
        let player_2_c = game_cheddar.next_player_account_id().clone();
        // near
        let player_1_n = game_near.current_player_account_id().clone();
        let player_2_n = game_near.next_player_account_id().clone();

        assert!(ctr.get_active_games().contains(&(game_id_cheddar, GameView::from(&game_cheddar))));
        assert!(ctr.get_active_games().contains(&(game_id_near, GameView::from(&game_near))));

        // near game
        // 600000000000
        // 600000000000

        make_move(&mut ctx, &mut ctr, &player_1_n, &game_id_near, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_2_n, &game_id_near, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_1_n, &game_id_near, 1, 1);
        make_move(&mut ctx, &mut ctr, &player_2_n, &game_id_near, 2, 2);
        make_move(&mut ctx, &mut ctr, &player_1_n, &game_id_near, 0, 2);
        make_move(&mut ctx, &mut ctr, &player_2_n, &game_id_near, 2, 0);
        make_move(&mut ctx, &mut ctr, &player_1_n, &game_id_near, 2, 1);


        let player_1_stats = ctr.get_stats(&opponent2);
        let player_2_stats = ctr.get_stats(&user2);
        println!("{:#?}", player_1_stats);
        println!("{:#?}", player_2_stats);
        //todo
        // assert!(
        //     player_1_stats.games_played == player_2_stats.games_played
        // );
        // assert!(
        //     player_2_stats.victories_num == 0 && player_1_stats.victories_num == 1
        // );   
        // assert_eq!(
        //     player_1_stats.total_reward.clone(), Vec::from([
        //         (
        //             "near".parse().unwrap(), 
        //             (2 * ONE_NEAR - ((2 * ONE_NEAR / BASIS_P as u128 )* MIN_FEES as u128))
        //         )
        //     ])
        // );
        // assert!(player_2_stats.total_reward.is_empty());

        testing_env!(ctx
            .predecessor_account_id(player_2_c)
            .block_timestamp(sec_to_nano(ctr.max_game_duration) + 1)
            .attached_deposit(ONE_YOCTO)
            .build()
        );

        ctr.claim_timeout_win(&game_id_cheddar);
        Ok((ctx, ctr))
    }

    #[test]
    fn test_near_deposit() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
        assert!(ctr.get_available_players().is_empty());
        make_available_near(&mut ctx, &mut ctr, &user(), ONE_NEAR, None, Some(referrer()), AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_NEAR, Some(user()), None, AVAILABLE_FOR_DEFAULT);
    }

    #[test]
    fn make_available_unavailable_near() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
        assert!(ctr.get_available_players().is_empty());
        make_available_near(&mut ctx, &mut ctr, &user(), ONE_NEAR, None, Some(referrer()), AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_NEAR, Some(user()), None, AVAILABLE_FOR_DEFAULT);
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfigView)>::from([
            (user(), GameConfigView { 
                deposit: U128(ONE_NEAR), 
                opponent_id: None, 
                referrer_id: Some(referrer()),
                created_at: 0
            }),
            (opponent(), GameConfigView { 
                deposit: U128(ONE_NEAR), 
                opponent_id: Some(user()), 
                referrer_id: None,
                created_at: 0
            }),
        ]));
        make_unavailable(&mut ctx, &mut ctr, &user());
        make_unavailable(&mut ctx, &mut ctr, &opponent());
        assert!(ctr.get_available_players().is_empty());
    }
    #[test]
    fn test_make_available_unavailable() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfigView)>::from([
            (user(), GameConfigView {  
                deposit: U128(ONE_CHEDDAR), 
                opponent_id: Some(opponent()), 
                referrer_id: Some(referrer()),
                created_at: 0
            }),
            (opponent(), GameConfigView {  
                deposit: U128(ONE_CHEDDAR), 
                opponent_id: Some(user()), 
                referrer_id: None,
                created_at: 0
            }),
        ]));
        make_unavailable(&mut ctx, &mut ctr, &user());
        make_unavailable(&mut ctx, &mut ctr, &opponent());
        assert!(ctr.get_available_players().is_empty());
    }
    #[test]
    #[should_panic(expected="Mismatch deposit token! Both players have to deposit the same token to play the game")]
    fn start_game_diff_tokens() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
        assert!(ctr.get_available_players().is_empty());
        let gc1 = GameConfigArgs { 
            opponent_id: Some(opponent()), 
            referrer_id: Some(referrer()) 
        };
        let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");

        make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, None, None, AVAILABLE_FOR_DEFAULT);
        start_game(&mut ctx, &mut ctr, &user(), &opponent());
    }
    #[test]
    fn test_give_up() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(ctr.get_available_players(), Vec::<(AccountId, GameConfigView)>::from([
            (user(), GameConfigView { 
                deposit: U128(ONE_CHEDDAR), 
                opponent_id: Some(opponent()), 
                referrer_id: Some(referrer()),
                created_at: 0 
            }),
            (opponent(), GameConfigView {  
                deposit: U128(ONE_CHEDDAR), 
                opponent_id: Some(user()), 
                referrer_id: None,
                created_at: 0 
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
            player_2_stats.total_reward, (2 * ONE_CHEDDAR)
        );
        assert_eq!(player_1_stats.total_reward, 0);
    }
    #[test]
    fn test_game_basics() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(10), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        // _ O O _ X
        // _ O _ X _
        // _ O X _ _
        // _ X O _ _
        // X _ _ _ _

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 4);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 3);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 3, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 3, 3);
        let winner = make_move(&mut ctx, &mut ctr, &player_2, &game_id, 4, 0);
        assert_eq!(winner ,Some(GameResult::Win(player_2)));

        let player_1_stats = ctr.get_stats(&user());
        let player_2_stats = ctr.get_stats(&&opponent());
        println!("{:#?}", player_1_stats);
        println!("{:#?}", player_2_stats);
        assert!(
            player_1_stats.games_played == player_2_stats.games_played
        );
        assert!(
            player_1_stats.victories_num == 1 && player_2_stats.victories_num == 0
        );   
        assert_eq!(
            player_1_stats.total_reward.clone(), (2 * ONE_CHEDDAR - ((2 * ONE_CHEDDAR / BASIS_P as u128 )* 10)))
            ;
        assert_eq!(player_2_stats.total_reward, 0);
    }
    #[test]
    fn test_game_basics_2() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(10), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        // _ O O _ X
        // _ O _ X _
        // _ O X _ _
        // _ X O _ _
        // X _ _ _ _

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 4, 4);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 3);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 3, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 3, 1);
        print_tiles(&ctr.internal_get_game(&game_id).to_tiles());
        let winner = make_move(&mut ctx, &mut ctr, &player_1, &game_id, 4, 0);
        assert_eq!(winner,Some(GameResult::Win(player_1)));
    }

    #[test]
    fn test_game_basics_near() {
        assert!(game_basics().is_ok());
    }

    #[test]
    fn test_tie_scenario() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 3);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 4);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 0);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 3);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 4);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 2);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 3);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 4);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 3, 0);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 3, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 3, 2);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 3, 3);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 3, 4);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 4, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 4, 0);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 4, 3);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 4, 4);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 4, 2);

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
    }
    #[test]
    fn test_stop_game_too_early() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
        let result = ctr.claim_timeout_win(&game_id);
        assert_eq!(result, None)
    }

    #[test]
    #[should_panic(expected="No access")]
    fn test_stop_game_wrong_access() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);

        let player_not_in_the_game: AccountId = "not_in_game.near".parse().unwrap();
        testing_env!(ctx
            .predecessor_account_id(player_not_in_the_game)
            .block_timestamp(sec_to_nano(TIMEOUT_WIN + 1))
            .build()
        );
        ctr.claim_timeout_win(&game_id);
    }

    // #[test]
    // fn test_expired_game() {
    //     let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION_SEC));
    //     assert!(ctr.get_available_players().is_empty());
    //     let gc1 = GameConfigArgs { 
    //         opponent_id: Some(opponent()), 
    //         referrer_id: None 
    //     };
    //     let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
    //     let gc2 = GameConfigArgs { 
    //         opponent_id: Some(user()), 
    //         referrer_id: None 
    //     };
    //     let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
    //     make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
    //     make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
    //     let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
    //     let game = ctr.internal_get_game(&game_id);
    //     let player_1 = game.current_player_account_id().clone();
    //     let player_2 = game.next_player_account_id().clone();

    //     println!("( {} , {} )", player_1, player_2);

    //     assert_ne!(player_1, game.next_player_account_id());
    //     assert_ne!(game.players[0].piece, game.players[1].piece);
    //     assert_eq!(player_1, game.players[0].account_id);
    //     assert_eq!(player_2, game.players[1].account_id);
    //     assert_eq!(game.board.current_piece, game.players[0].piece);

    //     assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

    //     let mut make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
    //     print_tiles(&tiles);
    //     testing_env!(ctx
    //         .predecessor_account_id(player_2.clone())
    //         .block_timestamp(sec_to_nano(601))
    //         .build()
    //     );
    //     // player2 turn too slow
    //     ctr.make_move(&game_id, 1, 2);
    //     assert!(ctr.get_stats(&player_1).victories_num == 1);
    //     assert!(ctr.get_stats(&player_2).victories_num == 0);
    //     assert_eq!(
    //         ctr.get_stats(&player_1).total_reward,
    //         Vec::from([
    //             (
    //                 acc_cheddar(),
    //                 (2 * ONE_CHEDDAR - (2 * ONE_CHEDDAR / BASIS_P as u128 * MIN_FEES as u128)) 
    //             )
    //         ])
    //     )
    // }

    // #[test]
    // fn test_stop_game() {
    //     let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  None);
    //     assert!(ctr.get_available_players().is_empty());
    //     let gc1 = GameConfigArgs { 
    //         opponent_id: Some(opponent()), 
    //         referrer_id: None 
    //     };
    //     let msg1 = near_sdk::serde_json::to_string(&gc1).expect("err serialize");
    //     let gc2 = GameConfigArgs { 
    //         opponent_id: Some(user()), 
    //         referrer_id: None 
    //     };
    //     let msg2 = near_sdk::serde_json::to_string(&gc2).expect("err serialize");
    //     make_available_ft(&mut ctx, &mut ctr, &user(), ONE_CHEDDAR, msg1);
    //     make_available_ft(&mut ctx, &mut ctr, &opponent(), ONE_CHEDDAR, msg2);
        
    //     let game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        
    //     let game = ctr.internal_get_game(&game_id);
    //     let player_1 = game.current_player_account_id().clone();
    //     let player_2 = game.next_player_account_id().clone();

    //     println!("( {} , {} )", player_1, player_2);

    //     assert_ne!(player_1, game.next_player_account_id());
    //     assert_ne!(game.players[0].piece, game.players[1].piece);
    //     assert_eq!(player_1, game.players[0].account_id);
    //     assert_eq!(player_2, game.players[1].account_id);
    //     assert_eq!(game.board.current_piece, game.players[0].piece);

    //     assert!(ctr.get_active_games().contains(&(game_id, game.clone())));

    //     let mut make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 0);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_1, &game_id, 2, 1);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 2, 2);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_1, &game_id, 1, 0);
    //     print_tiles(&tiles);
    //     make_move(&mut ctx, &mut ctr, &player_2, &game_id, 1, 2);
    //     print_tiles(&tiles);
        
    //     stop_game(&mut ctx, &mut ctr, &player_2, &game_id, 601);
        
    //     let player_1_stats = ctr.get_stats(&opponent());
    //     let player_2_stats = ctr.get_stats(&user());

    //     assert!(
    //         player_1_stats.games_played == player_2_stats.games_played
    //     );
    //     assert!(
    //         player_2_stats.victories_num == 1 && player_1_stats.victories_num == 0
    //     );
    //     assert!(
    //         !player_2_stats.total_reward.is_empty() && player_1_stats.total_reward.is_empty()
    //     ); 
    //     assert_eq!(
    //         player_2_stats.total_reward,
    //         Vec::from([
    //             (
    //                 acc_cheddar(),
    //                 (2 * ONE_CHEDDAR - (2 * ONE_CHEDDAR / BASIS_P as u128 * MIN_FEES as u128)) 
    //             )
    //         ])
    //     )
    // }

    #[test]
    fn test_new_views() -> Result<(), std::io::Error>{
        let (mut ctx, mut ctr) = game_basics()?;

        println!("ContractParams: {:#?}", ctr.get_contract_params());
        println!("TotalStatsNum: {:#?}", ctr.get_total_stats_num());
        println!("AccountsPlayed: {:#?}", ctr.get_accounts_played());
        println!("UserPenalties: {:#?}", ctr.get_user_penalties(&user()));

        println!("PenaltyUsers: {:#?}", ctr.get_penalty_users());

        make_available_near(&mut ctx, &mut ctr, &user(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &"third".parse().unwrap(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);

        assert_eq!(ctr.get_available_players().len(), 3);
        testing_env!(ctx
            .block_timestamp(sec_to_nano(ctr.max_game_duration + MAX_TIME_TO_BE_AVAILABLE))
            .build()
        );
        assert_eq!(ctr.get_available_players().len(), 3);

        // test ping expired players
        testing_env!(ctx
            .block_timestamp(sec_to_nano(ctr.max_game_duration + MAX_TIME_TO_BE_AVAILABLE + 2))
            .build()
        );
        print!("here\\");
        make_available_near(&mut ctx, &mut ctr, &"fourth".parse().unwrap(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);

        assert_eq!(ctr.get_available_players().len(), 1);
        assert_eq!(ctr.get_available_players()[0].0, "fourth".parse().unwrap());


        make_available_near(&mut ctx, &mut ctr, &user(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &opponent(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);
        make_available_near(&mut ctx, &mut ctr, &"third".parse().unwrap(), ONE_NEAR, None, None, AVAILABLE_FOR_DEFAULT);

        // first game starts at (max_game_duration + MAX_TIME_TO_BE_AVAILABLE +2) timestamp
        let first_game_id = start_game(&mut ctx, &mut ctr, &user(), &opponent());
        let first_game = ctr.internal_get_game(&first_game_id); 
        let current_player_first_game = first_game.current_player_account_id();
        let next_player_first_game = first_game.next_player_account_id();
        
        let future_winner_stats = ctr.get_stats(&next_player_first_game);
        let future_looser_stats = ctr.get_stats(&current_player_first_game);

        let winner_num_wins = future_winner_stats.victories_num;
        let looser_num_penalties = future_looser_stats.penalties_num;
        
        // second game starts 12 minutes after first
        testing_env!(ctx
            .block_timestamp(sec_to_nano(ctr.max_game_duration + MAX_TIME_TO_BE_AVAILABLE + 2 + ctr.max_turn_duration * 25 + 1))
            .build()
        );

        println!("game duration max - {}", ctr.max_game_duration);
        println!("turn duration max - {}", ctr.max_turn_duration);

        let second_game_id = start_game(&mut ctx, &mut ctr, &"third".parse().unwrap(), &"fourth".parse().unwrap());
        
        assert_eq!(ctr.get_active_games().len(), 3);

        let mut second_game = ctr.internal_get_game(&second_game_id); 
        let current_player_second_game = second_game.current_player_account_id();
        let next_player_second_game = second_game.next_player_account_id();
        print!("game init at:{}, max_turn_duration: {}", second_game.initiated_at, ctr.max_turn_duration);
        testing_env!(ctx
            .block_timestamp(sec_to_nano(second_game.initiated_at + ctr.max_turn_duration - 1))
            .build()
        );
        make_move(&mut ctx, &mut ctr, &current_player_second_game, &second_game_id, 0, 0);
        second_game = ctr.internal_get_game(&second_game_id); 

        testing_env!(ctx
            .block_timestamp(sec_to_nano(second_game.initiated_at + (ctr.max_turn_duration - 1) + (ctr.max_turn_duration - 1)))
            .build()
        );
        make_move(&mut ctx, &mut ctr, &next_player_second_game, &second_game_id, 0, 1);

        assert_eq!(
            ctr.get_active_games().len(), 1, 
            "first and second games need to be removed (expired) after max_game_duration passed for this"
        );
        assert_eq!(
            ctr.get_active_games()[0].0, second_game_id,
            "first and second games needs to be removed (expired) after max_game_duration passed for this"
        );

        println!("ContractParams: {:#?}", ctr.get_contract_params());

        let new_future_winner_stats = ctr.get_stats(&next_player_first_game);
        let new_future_looser_stats = ctr.get_stats(&current_player_first_game);

        let new_winner_num_wins = new_future_winner_stats.victories_num;
        let new_looser_num_penalties = new_future_looser_stats.penalties_num;
        
        assert!(new_winner_num_wins - winner_num_wins == 1);
        assert!(new_looser_num_penalties - looser_num_penalties == 1);

        assert!(
            ctr.get_penalty_users()
                .iter()
                .map(|(acc, _)| acc.clone())
                .collect::<Vec<AccountId>>()
                .contains(&current_player_first_game)
        );

        Ok(())
    }
    #[test]
    fn test_claim_timeout_win() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        testing_env!(ctx
            .predecessor_account_id(player_1.clone())
            .block_timestamp(sec_to_nano(TIMEOUT_WIN + 1).into())
            .build()
        );
        // player2 turn too slow
        ctr.claim_timeout_win(&game_id);
        assert!(ctr.get_stats(&player_1).victories_num == 1);
        assert!(ctr.get_stats(&player_2).victories_num == 0);
        assert_eq!(ctr.get_stats(&player_1).total_reward,(2 * ONE_CHEDDAR));
    }
    #[test]
    fn test_claim_timeout_win_when_no_timeout() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(10), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));

        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 2);
        testing_env!(ctx
            .predecessor_account_id(player_1.clone())
            .block_timestamp((TIMEOUT_WIN - 1).into())
            .build()
        );
        // player2 turn still have time left to make a move -> dont change anything just log that the claim is not valid yet 
        ctr.claim_timeout_win(&game_id);
        assert!(game.game_state == GameState::Active);
    }
    #[test] 
    fn test_player_piece_binding() {
        let game = Game::create_game(1, user(), opponent(), GameDeposit {balance: U128(5000) });
        assert_eq!(game.current_piece, Piece::O);
    }
    #[test]
    fn test_get_last_move() {
        let (mut ctx, mut ctr) = setup_contract(user(), Some(MIN_FEES), None,  Some(MIN_GAME_DURATION));
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
        assert_eq!(player_1, game.players.0);
        assert_eq!(player_2, game.players.1);
        assert_eq!(game.current_piece, Piece::O);

        assert!(ctr.get_active_games().contains(&(game_id, GameView::from(&game))));
        make_move(&mut ctx, &mut ctr, &player_1, &game_id, 0, 0);
        let (_last_move, last_piece) = get_last_move(&mut ctx, &mut ctr, &player_1, &game_id);
        let last_move = _last_move.unwrap();
        assert_eq!(last_move.x, 0);
        assert_eq!(last_move.y, 0);
        assert_eq!(last_piece, Piece::O);
        make_move(&mut ctx, &mut ctr, &player_2, &game_id, 0, 1);
        let (_last_move, last_piece) = get_last_move(&mut ctx, &mut ctr, &player_2, &game_id);
        let last_move = _last_move.unwrap();
        assert_eq!(last_move.x, 1);
        assert_eq!(last_move.y, 0);
        assert_eq!(last_piece, Piece::X);
        
    }
}
