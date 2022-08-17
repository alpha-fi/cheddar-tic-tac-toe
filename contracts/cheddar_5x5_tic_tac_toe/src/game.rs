use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum GameState {
    NotStarted,
    Active,
    Finished
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

#[derive(BorshSerialize, BorshDeserialize, Clone, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct Game {
    pub game_state: GameState,
    pub players: Vec<Player>,
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
        player_1: AccountId,
        player_2: AccountId,
        reward: GameDeposit
    ) -> Game {
        assert_ne!(player_1, player_2, "Player 1 and Player 2 have the same AccountId: @{}", &player_1);
        let (player_1, player_2) = Game::create_players(player_1, player_2);
        let board = Board::new(&player_1, &player_2);
        let mut game = Game { 
            game_state: GameState::NotStarted, 
            players:Vec::with_capacity(PLAYERS_NUM),
            current_piece: player_1.piece,
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
    /// creates random piece for player1 and `other()` one for player2
    fn create_players(account_id_1: AccountId, account_id_2: AccountId) -> (Player, Player) {
        let piece_1 = Piece::random();
        let piece_2 = piece_1.other();
        (
            Player::new(piece_1, account_id_1),
            Player::new(piece_2, account_id_2)
        )
    }
    /// set two players in `game.players`
    /// directly in order: [player_1, player_2]
    fn set_players(&mut self, player_1: Player, player_2: Player) {
        self.players.push(player_1.clone());
        self.players.push(player_2.clone());

        assert_eq!(self.players[0], player_1.clone());
        assert_eq!(self.players[1], player_2.clone());

        assert_eq!(
            self.players[0].piece,
            self.board.current_piece,
            "Invalid game settings: First player's Piece mismatched on Game <-> Board"
        );
        assert_ne!(
            self.players[1].piece,
            self.board.current_piece,
            "Invalid game settings: Second player's Piece mismatched on Game <-> Board"
        );
        assert_ne!(
            self.players[0].piece, self.players[1].piece,
            "Players cannot have equal Pieces"
        )
    }

    pub fn change_state(&mut self, new_state: GameState) {
        assert_ne!(new_state, self.game_state, "State is already {:?}", new_state);
        self.game_state = new_state
    }

    pub fn get_player_acc_by_piece(&self, piece: Piece) -> Option<&AccountId> {
        if &piece == &self.players[0].piece {
            Some(&self.players[0].account_id)
        } else if &piece == &self.players[1].piece {
            Some(&self.players[1].account_id)
        } else {
            panic!("No account with associated piece {:?}", piece)
        }
    }

    pub fn get_player_accounts(&self) -> (AccountId, AccountId) {
        (self.current_player_account_id(), self.next_player_account_id())
    }

    pub fn current_player_account_id(&self) -> AccountId {
        let index = self.current_player_index as usize;
        self.players[index].account_id.clone()
    }

    pub fn next_player_account_id(&self) -> AccountId {
        let index = self.current_player_index as usize;
        self.players[1 - index].account_id.clone()
    }

    pub fn contains_player_account_id(&self, account_id: &AccountId) -> bool {
        if &self.current_player_account_id() == account_id || &self.next_player_account_id() == account_id {
            true
        } else {
            false
        }
    }
    pub fn reward(&self) -> GameDeposit {
        self.reward.clone()
    }
    
}