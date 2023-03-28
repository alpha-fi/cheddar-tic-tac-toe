use crate::*;
use std::collections::HashMap;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum GameResult {
    Win(AccountId),
    Tie,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct Tiles {
    pub o_coords: Vec<Coords>,
    pub x_coords: Vec<Coords>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct ContractParams {
    pub games: HashMap<GameId, GameView>,
    pub available_players: Vec<(AccountId, GameConfigView)>,
    pub service_fee: u16,
    pub max_game_duration: u64,
    pub last_update_timestamp_sec: u64,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct GameView {
    pub player1: AccountId,
    pub player2: AccountId,
    pub game_status: GameState,
    pub current_player: AccountId,
    pub total_bet: GameDeposit,
    pub tiles: Tiles,
    /* * */
    pub initiated_at_sec: u64,
    pub last_turn_timestamp_sec: u64,
    pub current_duration_sec: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GameLimitedView {
    pub game_result: GameResult,
    pub player1: AccountId,
    pub player2: AccountId,
    pub reward_or_tie_refund: GameDeposit,
    pub tiles: Tiles,
    pub last_move: Option<(Coords, Piece)>,
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
        assert_ne!(
            current_player, opposit_player,
            "Error: Matched accounts for opposite players"
        );
        Self {
            player1: current_player,
            player2: opposit_player,
        }
    }
}

impl From<&Game> for GameView {
    fn from(g: &Game) -> Self {
        let (player1, player2) = g.get_player_accounts();
        let current_player: AccountId = match g.current_player_index {
            0 => g.players.0.clone(),
            _ => g.players.1.clone(),
        };
        Self {
            player1,
            player2,
            game_status: g.game_state.clone(),
            current_player,
            total_bet: g.reward(),
            tiles: g.to_tiles(),
            initiated_at_sec: g.initiated_at,
            last_turn_timestamp_sec: g.last_turn_timestamp,
            current_duration_sec: g.current_duration_sec,
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_contract_params(&mut self) -> ContractParams {
        let games: HashMap<u64, GameView> = self
            .games
            .iter()
            .map(|(game_id, game)| (game_id, GameView::from(&game)))
            .collect();
        let available_players = self.get_available_players();

        ContractParams {
            games,
            available_players,
            service_fee: self.service_fee,
            max_game_duration: nano_to_sec(self.max_game_duration_sec),
            last_update_timestamp_sec: nano_to_sec(self.last_update_timestamp),
        }
    }

    pub fn get_game(&self, game_id: &GameId) -> GameLimitedView {
        self.stored_games.get(game_id).expect("Game not found")
    }

    pub fn get_ordered_players(&self, game_id: &GameId) -> RangedPlayersView {
        self.games
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

    pub fn get_current_tiles(&self, game_id: &GameId) -> Tiles {
        let game = self.internal_get_game(game_id);
        game.to_tiles()
    }

    pub fn get_token_min_deposit(&self) -> U128 {
        self.min_deposit.into()
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

        let result: Vec<(_, _)> = accounts_played
            .iter()
            .map(|acc| {
                let penalties_by_acc = self.get_user_penalties(acc);
                (acc.clone(), penalties_by_acc)
            })
            .filter(|(_, penalties_by_acc)| penalties_by_acc.penalties_num > 0)
            .collect();

        result
    }

}
