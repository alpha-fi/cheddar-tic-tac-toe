use core::panic;

use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum GameState {
    NotStarted,
    Active,
    Finished,
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
    pub board: Board,
    pub total_turns: u8,
    pub initiated_at: u64,
    pub last_turn_timestamp: u64,
    pub current_duration: Duration,
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
        let board = Board::new(game_id);
        let mut game = Game {
            game_state: GameState::NotStarted,
            players: (player_1.clone(), player_2.clone()),
            current_piece: Piece::O,
            // player_1 index is 0
            current_player_index: 0,
            reward,
            board,
            total_turns: 0,
            initiated_at: env::block_timestamp(),
            last_turn_timestamp: 0,
            current_duration: 0,
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
            Piece::O, self.board.current_piece,
            "Invalid game settings: First player's Piece mismatched on Game <-> Board"
        );
        assert_ne!(
            Piece::X, self.board.current_piece,
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

    pub fn claim_timeout_win(&self, player: AccountId) -> bool {
        //1. Check if the game is still going
        assert_eq!(
            self.game_state,
            GameState::Active,
            "The game is already over!"
        );
        //2. Check if opponets move
        assert_ne!(
            player,
            self.current_player_account_id(),
            "Can't claim timeout win if it's your turn"
        );
        //3. Check for timeout
        let cur_timestamp = env::block_timestamp();
        if cur_timestamp - self.last_turn_timestamp <= utils::TIMEOUT_WIN {
            return false;
        }
        true
    }
}
