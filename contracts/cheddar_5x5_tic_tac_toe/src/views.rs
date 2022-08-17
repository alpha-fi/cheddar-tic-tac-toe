use crate::*;
use std::collections::HashMap;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum GameResult {
    Win(AccountId),
    Tie
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ContractParams {
    pub games: HashMap<GameId, GameView>,
    pub available_players: Vec<(AccountId, GameConfigView)>,
    /* * */
    pub service_fee_percentage: u32,
    pub max_game_duration: u32,
    pub last_update_timestamp_sec: u32
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameView {
    pub player1: AccountId,
    pub player2: AccountId,
    pub game_status: GameState,
    pub current_player: Player,
    pub reward: GameDeposit,
    pub tiles: [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE],
    /* * */
    pub initiated_at_sec: u32,
    pub last_turn_timestamp_sec: u32,
    pub current_duration_sec: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GameLimitedView {
    pub game_result: GameResult,
    pub player1: AccountId,
    pub player2: AccountId,
    pub reward_or_tie_refund: GameDeposit,
    pub board: [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE],
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RangedPlayersView {
    pub player1: AccountId,
    pub player2: AccountId,
}

impl From<&Game> for RangedPlayersView {
    fn from(g: &Game) -> Self {
        let current_player = g.current_player_account_id();
        let opposit_player = g.next_player_account_id();
        assert_ne!(current_player, opposit_player, "Error: Matched accounts for opposite players");
        Self { 
            player1: current_player, 
            player2: opposit_player, 
        }
    }
}

impl From<&Game> for GameView {
    fn from(g: &Game) -> Self {
        let (player1, player2) = g.get_player_accounts();
        let current_player = g.players[g.current_player_index as usize].clone();
        Self { 
            player1, 
            player2, 
            game_status: g.game_state,
            current_player,
            reward: g.reward(),
            tiles: g.board.tiles,
            initiated_at_sec: nano_to_sec(g.initiated_at),
            last_turn_timestamp_sec: nano_to_sec(g.last_turn_timestamp),
            current_duration_sec: nano_to_sec(g.current_duration),
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_contract_params(&self) -> ContractParams {
        let games:HashMap<u64, GameView> = self.games.iter()
            .map(|(game_id,game)| (game_id, GameView::from(&game)))
            .collect();
        let available_players = self.get_available_players();

        ContractParams { 
            games, 
            available_players, 
            service_fee_percentage: self.service_fee_percentage, 
            max_game_duration: nano_to_sec(self.max_game_duration),
            last_update_timestamp_sec: nano_to_sec(self.last_update_timestamp)
        } 
    }

    pub fn get_game(&self, game_id: &GameId) -> GameLimitedView {
        self.stored_games.get(game_id).expect("Game not found")
    }

    pub fn get_ordered_players(&self, game_id: &GameId) -> RangedPlayersView {
        self
            .games
            .get(game_id)
            .map(|game| RangedPlayersView::from(&game))
            .expect("Game was not found")
    }

    pub fn get_current_player(&self, game_id: &GameId) -> AccountId {
        self.internal_get_game(game_id).current_player_account_id()
    }

    pub fn get_next_player(&self, game_id: &GameId) -> AccountId {
        self.internal_get_game(game_id).next_player_account_id()
    }

    pub fn get_last_games(&self) -> Vec<(GameId, GameLimitedView)> {
        self.stored_games.to_vec()
    }

    pub fn get_current_tiles(&self, game_id: &GameId) -> [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE]{
        let game = self.internal_get_game(game_id);
        game.board.tiles
    }

    pub fn get_whitelisted_tokens(&self) -> Vec<(TokenContractId, U128)> {
        self.whitelisted_tokens
            .to_vec()
            .iter()
            .map(|(acc, min_dep)| (acc.clone(), U128(*min_dep)))
            .collect()
    }

    pub fn get_token_min_deposit(&self, token_id: &TokenContractId) -> U128 {
        self.whitelisted_tokens
            .get(token_id)
            .expect("Token isn't whitelisted")
            .into()
    }

    pub fn get_available_players(&self) -> Vec<(AccountId, GameConfigView)> {
        self.available_players
            .to_vec()
            .iter()
            .map(|(acc, game_config)| (acc.clone(), GameConfigView::from(game_config)))
            .collect()
    }

    pub fn get_active_games(&self) -> Vec<(GameId, GameView)> {
        self.games
            .to_vec()
            .iter()
            .map(|(game_id, game)| (*game_id, GameView::from(game)))
            .collect()
    }

    pub fn get_penalty_users(&self) -> Vec<(AccountId, UserPenalties)> {
        let accounts_played = self.get_accounts_played();
        assert_eq!(accounts_played.len() as u32, self.get_total_stats_num());

        let result: Vec<(_,_)> = accounts_played.iter()
            .map(|acc| {
                    let penalties_by_acc = self.get_user_penalties(acc);
                    (acc.clone(), penalties_by_acc)
                }
            )
            .filter(|(_, penalties_by_acc)| penalties_by_acc.penalties_num > 0)
            .collect();

        result
    }
}