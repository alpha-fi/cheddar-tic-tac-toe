use std::cmp::{min, max};

use near_sdk::serde;

use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub enum Winner {
    X,
    O,
    Tie,
}

#[derive(Debug, Clone)]
pub enum MoveError {
    /// The game was already over when a move was attempted
    GameAlreadyOver,
    /// The position provided was invalid
    InvalidPosition { row: usize, col: usize },
    /// The tile already contained another piece
    TileFilled {
        other_piece: Piece,
        row: usize,
        col: usize,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct Coords {
    pub x: u8,
    pub y: u8,
}

#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
// #[serde(crate = "near_sdk::serde")]
pub struct Board {
    // TODO: should be HashMap<Coords, Piece>
    pub(crate) tiles: UnorderedMap<Coords, Piece>,
    /// X or O: who is currently playing
    pub(crate) current_piece: Piece,
    pub(crate) winner: Option<Winner>,
}

impl Board {
    /// there is two players with different AccountId's and
    /// with different random given pieces
    /// accounts check in `Game.create_game`
    pub fn new(player_1: &Player, player_2: &Player) -> Self {
        assert_ne!(
            player_1.piece, player_2.piece,
            "players have same pieces: {:?}",
            player_1.piece
        );
        Self {
            tiles: UnorderedMap::new(b"m"),
            current_piece: player_1.piece,
            winner: None,
        }
    }
    pub fn check_move(&self, row: usize, col: usize) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameAlreadyOver);
        }
        if row > BOARD_SIZE || col > BOARD_SIZE {
            return Err(MoveError::InvalidPosition { row, col });
        }
        // Move in already filled tile
        else if let Some(other_piece) = self.tiles.get(&Coords { x: col as u8, y: row as u8 }) {
            return Err(MoveError::TileFilled {
                other_piece,
                row,
                col,
            });
        }
        Ok(())
    }
    pub fn check_winner(&self, position: Coords) -> bool {
        let expected = Some(self.current_piece);
        let mut c: Coords = Coords { x: position.x.clone(), y: position.y.clone() };
        let mut counter = 1;

        // 1. check rows
        // go max 4 pos to the left and see how far we can go
        for i in 1..=min(4, position.x) {
            c.x = c.x - 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        for i in 1..=max(4, BOARD_SIZE - 1 - position.x as usize) {
            c.x = c.x + 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }
        // 2. check collumns 
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        counter = 1;

        for i in 1..=min(4, position.y) {
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        for i in 1..=max(4, BOARD_SIZE - 1 - position.y as usize) {
            c.y = c.y + 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
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
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        counter = 1;
        for i in 1..=min(4, min(position.x, position.y)) {
            c.x = c.x - 1;
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        for i in 1..=max(4, BOARD_SIZE - 1 - max(position.x as usize, position.y as usize)) {
            c.x = c.x + 1;
            c.y = c.y + 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }

        //4. check diagonal (NE - SW)
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        counter = 1;
        for i in 1..=min(4, min(position.x, position.y)) {
            c.x = c.x + 1;
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = Coords { x: position.x.clone(), y: position.y.clone() };
        for i in 1..=max(4, BOARD_SIZE - 1 - max(position.x as usize, position.y as usize)) {
            c.x = c.x - 1;
            c.y = c.y + 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }

        // 5. Check if board is filled -> Tie
        if self.tiles.len() >= (BOARD_SIZE * BOARD_SIZE) as u64{
            return true; 
        }
        false
    }
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    pub fn update_winner(&mut self, coords: Coords) {
        if self.check_winner(coords) {
            if self.current_piece == Piece::X{
                self.winner =  Some(Winner::X);
            } else if self.current_piece == Piece::O{
                self.winner =  Some(Winner::O);
            } else {
                self.winner = Some(Winner::Tie);
            }
        }
    }
}
