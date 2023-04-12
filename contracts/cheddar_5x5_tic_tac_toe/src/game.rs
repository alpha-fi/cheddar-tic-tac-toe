use core::panic;
use std::cmp::{min, max};

use crate::{*, views::Tiles};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum GameState {
    NotStarted,
    Active,
    Finished,
}
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub enum Winner {
    X,
    O,
    Tie,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MoveError {
    /// The game was already over when a move was attempted
    GameOver,
    /// The position provided was invalid
    InvalidPosition { row: u8, col: u8 },
    /// The tile already contained another piece
    TileFilled {
        other_piece: Piece,
        row: u8,
        col: u8,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct Coords {
    pub x: u8,
    pub y: u8,
}

/// Deposit into `Game` for each `Player`
/// Used for computing reward
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameDeposit {
    pub token_id: TokenContractId,
    pub balance: U128,
}

#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct Game {
    pub game_state: GameState,
    pub players: (AccountId, AccountId),
    pub current_piece: Piece,
    pub current_player_index: u8,
    pub reward: GameDeposit,
    pub total_turns: u8,
    pub initiated_at: Timestamp,
    pub last_turn_timestamp: Timestamp,
    pub current_duration_sec: Duration,
    //board fields
    pub last_move: Option<Coords>,
    pub winner: Option<Winner>,
    pub board: UnorderedMap<Coords, Piece>,
    pub duration: Duration,
}

impl Game {
    /// set players in given order. First player (`player_1`)
    /// will have first move
    /// It generates randomly in `Contract.start_game` because
    /// first move gives more chances to win
    pub fn create_game(
        game_id: GameId,
        player_1: AccountId,
        player_2: AccountId,
        reward: GameDeposit,
    ) -> Game {
        assert_ne!(
            player_1, player_2,
            "Player 1 and Player 2 have the same AccountId: @{}",
            &player_1
        );
        let mut game = Game {
            game_state: GameState::NotStarted,
            players: (player_1.clone(), player_2.clone()),
            current_piece: Piece::O,
            // player_1 index is 0
            current_player_index: 0,
            reward,
            total_turns: 0,
            initiated_at: nano_to_sec(env::block_timestamp()),
            last_turn_timestamp: 0,
            current_duration_sec: 0,
            //board fields
            last_move: None,
            winner: None,
            board: UnorderedMap::new(StorageKey::GameBoard { game_id }),
            duration: 0,
        };
        game.set_players(player_1, player_2);
        game
    }
    /// set two players in `game.players`
    /// directly in order: [player_1, player_2]
    fn set_players(&mut self, player_1: AccountId, player_2: AccountId) {
        self.players.0 = player_1.clone();
        self.players.1 = player_2.clone();

        assert_eq!(self.players.0, player_1.clone());
        assert_eq!(self.players.1, player_2.clone());

        assert_eq!(
            Piece::O, self.current_piece,
            "Invalid game settings: First player's Piece mismatched on Game <-> Board"
        );
        assert_ne!(
            Piece::X, self.current_piece,
            "Invalid game settings: Second player's Piece mismatched on Game <-> Board"
        );
    }

    pub fn change_state(&mut self, new_state: GameState) {
        assert_ne!(
            new_state, self.game_state,
            "State is already {:?}",
            new_state
        );
        self.game_state = new_state
    }

    pub fn get_player_acc_by_piece(&self, piece: Piece) -> Option<&AccountId> {
        if piece == Piece::O {
            Some(&self.players.0)
        } else if piece == Piece::X {
            Some(&self.players.1)
        } else {
            panic!("No account with associated piece {:?}", piece)
        }
    }

    pub fn get_player_accounts(&self) -> (AccountId, AccountId) {
        (self.players.0.clone(), self.players.1.clone())
    }

    pub fn current_player_account_id(&self) -> AccountId {
        return match self.current_player_index {
            0 => self.players.0.clone(),
            _ => self.players.1.clone()
        };
    }

    pub fn player_piece(index: u8) -> Piece {
        return match index {
            0 => Piece::O,
            _ => Piece::X
        };
    }

    pub fn next_player_account_id(&self) -> AccountId {
        return match self.current_player_index {
            1 => self.players.0.clone(),
            _ => self.players.1.clone()
        };
    }

    pub fn contains_player_account_id(&self, user: &AccountId) -> bool {
        self.players.0 == *user || self.players.1 == *user
    }
    pub fn reward(&self) -> GameDeposit {
        self.reward.clone()
    }

    pub fn get_opponent(&self, player: &AccountId) -> AccountId {
        if *player == self.current_player_account_id() {
            return self.next_player_account_id();
        } else {
            return self.current_player_account_id();
        }
    }

    pub fn claim_timeout_win(&self, player: &AccountId) -> bool {
        //1. Check if the game is still going
        assert_eq!(
            self.game_state,
            GameState::Active,
            "The game is already over!"
        );
        //2. Check if opponets move
        assert_ne!(
            *player,
            self.current_player_account_id(),
            "Can't claim timeout win if it's your turn"
        );
        //3. Check if the player invoking the method is in the game
        assert!(self.contains_player_account_id(&player), "No access");
        //4. Check for timeout
        let cur_timestamp = nano_to_sec(env::block_timestamp());
        if cur_timestamp - self.last_turn_timestamp <= utils::TIMEOUT_WIN_SEC {
            return false;
        }
        true
        return cur_timestamp - self.last_turn_timestamp > utils::TIMEOUT_WIN;
    }

    pub fn get_winner(&self) -> Option<GameResult> {
        self.board.winner.as_ref().map(|w| match w {
                Winner::O => GameResult::Win(self.players.0.clone()),
                Winner::X => GameResult::Win(self.players.1.clone()),
                Winner::Tie => GameResult::Tie
        })
    }

    pub fn get_winner(&self) -> Option<GameResult> {
        if self.winner.is_some() {
            return match self.winner.clone().unwrap() {
                Winner::O => Some(GameResult::Win(self.players.0.clone())),
                Winner::X => Some(GameResult::Win(self.players.1.clone())),
                Winner::Tie => Some(GameResult::Tie),
            }
        } else {
            return None;
        }
    }
    
    // board methods
    pub fn check_move(&self, coords: &Coords) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameOver);
        }
        if coords.y >= BOARD_SIZE || coords.x >= BOARD_SIZE {
            return Err(MoveError::InvalidPosition {
                row: coords.y,
                col: coords.x,
            });
        }
        // Move in already filled tile
        else if let Some(other_piece) = self.board.get(&coords) {
            return Err(MoveError::TileFilled {
                other_piece,
                row: coords.y,
                col: coords.x,
            });
        }
        Ok(())
    }
    pub fn check_winner(&self, position: &Coords) -> bool {
        let expected = Some(self.current_piece.other());
        let mut c = position.clone();
        let mut counter = 1;
        // 1. check rows
        // go max 4 pos to the left and see how far we can go
        for _ in 1..=min(4, position.x) {
            c.x = c.x - 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for _ in 1..=max(4, BOARD_SIZE - 1 - position.x) {
            c.x = c.x + 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }
        // 2. check columns
        c = position.clone();
        counter = 1;
        for _ in 1..=min(4, position.y) {
            c.y = c.y - 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for _ in 1..=max(4, BOARD_SIZE - 1 - position.y) {
            c.y = c.y + 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }
        // 3. check diagonal (NW - SE)
        // eg. o X o o x o
        //     x o X o o x
        //     o x x X o o
        //     o o o x X x
        //     o o o x x X
        //     x x o x o x
        c = position.clone();
        counter = 1;
        for _ in 1..=min(4, min(position.x, position.y)) {
            c.x = c.x - 1;
            c.y = c.y - 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for _ in 1..=max(4, BOARD_SIZE - 1 - max(position.x, position.y)) {
            c.x = c.x + 1;
            c.y = c.y + 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }

        //4. check diagonal (NE - SW)
        c = position.clone();
        let mut counter = 1;
        for _ in 1..=position.y {
            c.x = c.x + 1;
            c.y = c.y - 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for _ in 1..=position.x {
            c.x = c.x - 1;
            c.y = c.y + 1;
            if self.board.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }
        false
    }
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    pub fn update_winner(&mut self, coords: &Coords) {
        if self.board.len() >= MAX_NUM_TURNS {
            self.winner = Some(Winner::Tie);
            return;
        }
        if self.check_winner(coords) {
            if self.current_piece.other() == Piece::X {
                self.winner = Some(Winner::X);
                print!("X is the winner");
            } else if self.current_piece.other() == Piece::O {
                self.winner = Some(Winner::O);
                print!("O is the winner");
            }
        }
    }

    pub fn to_tiles(&self) -> Tiles {
        let mut o_coords = Vec::new();
        let mut x_coords = Vec::new();
        for (c, p) in self.board.iter() {
            match p {
                Piece::O => o_coords.push(c),
                Piece::X => x_coords.push(c),
            }
        }
        return Tiles { o_coords, x_coords };
    }
    pub fn get_last_move(&self) -> Option<Coords> {
        return self.last_move.clone();
    }
    pub fn get_last_move_piece(&self) -> Piece {
        if self.current_piece == Piece::O {
            return Piece::X;
        } else {
            return Piece::O;
        }

    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use near_sdk::{AccountId, json_types::U128};

    use crate::{utils::BOARD_SIZE, game::MoveError, player::Piece};

    use super::{Game, GameDeposit, Coords};

    fn user() -> AccountId {
        "user".parse().unwrap()
    }
    fn opponent() -> AccountId {
        "opponent.near".parse().unwrap()
    }
    fn acc_cheddar() -> AccountId {
        "cheddar".parse().unwrap()
    }
    pub fn init_game() -> Game {
        return Game::create_game(1, user(), opponent(), GameDeposit{token_id: acc_cheddar(), balance: U128(50000)});
    }
    #[test]
    fn valid_move() {
        
        // initialize the board
        let game = init_game();

        // make move
        let _ = game.check_move(&Coords { x: 0, y: 0 });
    }
    #[test]
    fn index_out_of_bound() {
        
        // initialize the board
        let game = init_game();
        // make move
        let result = game.check_move(&Coords {
            x: BOARD_SIZE,
            y: BOARD_SIZE,
        });
        assert_eq!(
            result,
            Err(MoveError::InvalidPosition {
                row: BOARD_SIZE,
                col: BOARD_SIZE
            })
        );
    }
    #[test]
    fn alreddy_taken_field() {
        // create random Piece
        let piece_1 = Piece::random();
        
        // initialize the board
        let mut game = init_game();

        // make move
        game.board.insert(&Coords { x: 0, y: 0 }, &piece_1);
        let result = game.check_move(&Coords { x: 0, y: 0 });
        assert_eq!(
            result,
            Err(MoveError::TileFilled {
                other_piece: piece_1,
                row: 0,
                col: 0
            })
        );
    }
    #[test]
    fn check_row_winner() {
        // create two players
        let piece_1 = Piece::O;
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // O O O O _
        // X X X X _
        // _ _ _ _ _
        // _ _ _ _ _
        // _ _ _ _ _
        game.board.insert(&Coords { x: 0, y: 0 }, &piece_1);
        game.board.insert(&Coords { x: 1, y: 0 }, &piece_1);
        game.board.insert(&Coords { x: 2, y: 0 }, &piece_1);
        game.board.insert(&Coords { x: 3, y: 0 }, &piece_1);
        game.board.insert(&Coords { x: 0, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 1 }, &piece_2);
        let result = game.check_winner(&Coords { x: 4, y: 1 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_1() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();
        // prepare the board
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // _ _ _ _ _
        game.board.insert(&Coords { x: 0, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 3 }, &piece_2);
        let result = game.check_winner(&Coords { x: 0, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_2() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // _ _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        game.board.insert(&Coords { x: 0, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 3 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 4 }, &piece_2);
        let result = game.check_winner(&Coords { x: 0, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_3() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // O _ _ _ _
        // O _ _ _ _
        // _ _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        game.board.insert(&Coords { x: 0, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 3 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 4 }, &piece_2);
        let result = game.check_winner(&Coords { x: 0, y: 2 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_1() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // O _ _ _ _
        // _ O _ _ _
        // _ _ O _ _
        // _ _ _ O _
        // _ _ _ _ _
        game.board.insert(&Coords { x: 0, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 3 }, &piece_2);
        let result = game.check_winner(&Coords { x: 4, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_2() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // _  _ _ _ _
        // _ O _ _ _
        // _ _ O _ _
        // _ _ _ O _
        // _ _ _ _ O
        game.board.insert(&Coords { x: 4, y: 4 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 3 }, &piece_2);
        let result = game.check_winner(&Coords { x: 0, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_3() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // O _ _ _ _
        // _ O _ _ _
        // _ _ _ _ _
        // _ _ _ O _
        // _ _ _ _ O
        game.board.insert(&Coords { x: 0, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 4, y: 4 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 3 }, &piece_2);
        let result = game.check_winner(&Coords { x: 2, y: 2 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_1() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // _ _ _ _ O
        // _ _ _ O _
        // _ _ O _ _
        // _ O _ _ _
        // _ _ _ _ _
        game.board.insert(&Coords { x: 4, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 3 }, &piece_2);
        let result = game.check_winner(&Coords { x: 0, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_2() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // _ _ _ _ _
        // _ _ _ O _
        // _ _ O _ _
        // _ O _ _ _
        // O _ _ _ _
        game.board.insert(&Coords { x: 0, y: 4 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 3 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 1 }, &piece_2);
        let result = game.check_winner(&Coords { x: 4, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_3() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        // _ _ _ _ O
        // _ _ _ O _
        // _ _ O _ _
        // _ _ _ _ _
        // O _ _ _ _
        game.board.insert(&Coords { x: 4, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 0, y: 4 }, &piece_2);
        let result = game.check_winner(&Coords { x: 1, y: 3 });
        assert_eq!(result, true);
    }
    #[test]
    fn test_board_25_x_25() {
        
        // initialize the board
        let board = init_game();

        // make move
        let mut result = board.check_move(&Coords {x: 24, y: 24});
        assert_eq!(result,Ok(()));
        result = board.check_move(&Coords {x: 24, y: 0});
        assert_eq!(result,Ok(()));
        result = board.check_move(&Coords {x: 0, y: 24});
        assert_eq!(result,Ok(()));
        result = board.check_move(&Coords {x: 0, y: 0});
        assert_eq!(result,Ok(()));
    }
    #[test]
    fn check_sw_diagonal_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        game.board.insert(&Coords { x: 24, y: 0}, &piece_2);
        game.board.insert(&Coords { x: 23, y: 1}, &piece_2);
        game.board.insert(&Coords { x: 22, y: 2}, &piece_2);
        game.board.insert(&Coords { x: 20, y: 4}, &piece_2);
        let result = game.check_winner(&Coords{ x: 21, y: 3 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_row_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        game.board.insert(&Coords { x: 24, y: 20}, &piece_2);
        game.board.insert(&Coords { x: 23, y: 20}, &piece_2);
        game.board.insert(&Coords { x: 22, y: 20}, &piece_2);
        game.board.insert(&Coords { x: 21, y: 20}, &piece_2);
        let result = game.check_winner(&Coords{ x: 20, y: 20 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_diagonal_se_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        game.board.insert(&Coords { x: 24, y: 24}, &piece_2);
        game.board.insert(&Coords { x: 23, y: 23}, &piece_2);
        game.board.insert(&Coords { x: 21, y: 21}, &piece_2);
        game.board.insert(&Coords { x: 20, y: 20}, &piece_2);
        let result = game.check_winner(&Coords{ x: 22, y: 22 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_horizontal_bottom_edge_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        game.board.insert(&Coords { x: 24, y: 20}, &piece_2);
        game.board.insert(&Coords { x: 24, y: 21}, &piece_2);
        game.board.insert(&Coords { x: 24, y: 22}, &piece_2);
        game.board.insert(&Coords { x: 24, y: 23}, &piece_2);
        let result = game.check_winner(&Coords{ x: 24, y: 24 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_horizontal_top_edge_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // prepare the board
        game.board.insert(&Coords { x: 24, y: 0}, &piece_2);
        game.board.insert(&Coords { x: 24, y: 1}, &piece_2);
        game.board.insert(&Coords { x: 24, y: 2}, &piece_2);
        game.board.insert(&Coords { x: 24, y: 3}, &piece_2);
        let result = game.check_winner(&Coords{ x: 24, y: 4 });
        assert_eq!(result, true); 
    }
    #[test]
    fn test_to_tiles() {
        let piece_2 = Piece::X;
        
        // initialize the board
        let mut game = init_game();

        // insert few testing values
        // _ _ _ _ X
        // _ _ _ X _
        // _ _ X _ _
        // _ X _ _ _
        // _ _ _ _ _
        game.board.insert(&Coords { x: 4, y: 0 }, &piece_2);
        game.board.insert(&Coords { x: 3, y: 1 }, &piece_2);
        game.board.insert(&Coords { x: 2, y: 2 }, &piece_2);
        game.board.insert(&Coords { x: 1, y: 3 }, &piece_2);
        let vector = game.to_tiles();
        assert_eq!(vector.o_coords.len(), 0);
        assert_eq!(vector.x_coords.len(), 4);
        assert_eq!(vector.x_coords[0], Coords { x: 4, y: 0 });
        assert_eq!(vector.x_coords[1], Coords { x: 3, y: 1 });
        assert_eq!(vector.x_coords[2], Coords { x: 2, y: 2 });
        assert_eq!(vector.x_coords[3], Coords { x: 1, y: 3 });
    }
}