use crate::*;
use std::collections::HashMap;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ContractParams {
    games: HashMap<GameId, GameView>,
    available_players: Vec<(AccountId, GameConfig)>,
    /* * */
    service_fee_percentage: String,
    max_game_duration: String
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct GameView {
    pub player1: AccountId,
    pub player2: AccountId,
    pub game_status: GameState
}

impl From<Game> for GameView {
    fn from(g: Game) -> Self {
        let (player1, player2) = g.get_player_accounts();
        Self { 
            player1, 
            player2, 
            game_status: g.game_state
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_contract_params(&self) -> ContractParams {
        let games:HashMap<u64, GameView> = self.games.iter()
            .map(|(game_id,game)| (game_id, GameView::from(game)))
            .collect();
        let available_players = self.available_players.to_vec();
        let service_fee_percentage = format!("{} %", f64::from(self.service_fee_percentage) / 100.0);
        let max_game_duration = format!("{} sec", nano_to_sec(self.max_game_duration));
        ContractParams { 
            games, 
            available_players, 
            service_fee_percentage, 
            max_game_duration
        } 
    }

    pub fn get_current_tiles(&self, game_id: &GameId) -> [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE]{
        let game = self.internal_get_game(game_id);
        game.board.tiles
    }
}