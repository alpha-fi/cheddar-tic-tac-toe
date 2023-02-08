use std::cmp::{max, min};
use std::collections::HashMap;

use crate::*;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
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
    InvalidPosition(Coords),
    /// The tile already contained another piece
    TileFilled {
        other_piece: Piece,
        row: u8,
        col: u8,
    },
}

// TODO: check why we need Eq (and PartialEq is not enough)
#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Eq, PartialEq, Clone, Hash, PartialOrd,
)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct Coords {
    pub x: u8,
    pub y: u8,
}

#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct Board {
    pub(crate) tiles: HashMap<Coords, Piece>,
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
            tiles: HashMap::new(),
            current_piece: player_1.piece,
            winner: None,
        }
    }
    pub fn check_move(&self, coords: Coords) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameAlreadyOver);
        }
        if coords.x >= BOARD_SIZE || coords.y >= BOARD_SIZE {
            return Err(MoveError::InvalidPosition {
                row: coords.y,
                col: coords.x,
            });
        }
        // Move in already filled tile
        else if let Some(other_piece) = self.tiles.get(&coords) {
            return Err(MoveError::TileFilled {
                other_piece,
                row: coords.x,
                col: coords.y,
            });
        }
        Ok(())
    }
    pub fn check_winner(&self, position: Coords) -> bool {
        let expected = Some(self.current_piece);
        let mut c = position.clone();
        let mut counter = 1;

        // 1. check rows
        // go max 4 pos to the left and see how far we can go
        for i in 1..=min(4, position.x) {
            c.x = c.x - 1;
            if self.tiles.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for i in 1..=max(4, BOARD_SIZE - 1 - position.x) {
            c.x = c.x + 1;
            if self.tiles.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }
        // 2. check collumns
        c = position.clone();
        counter = 1;

        for i in 1..=min(4, position.y) {
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for i in 1..=max(4, BOARD_SIZE - 1 - position.y) {
            c.y = c.y + 1;
            if self.tiles.get(&c) == expected {
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
        for i in 1..=min(4, min(position.x, position.y)) {
            c.x = c.x - 1;
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for i in 1..=max(4, BOARD_SIZE - 1 - max(position.x, position.y)) {
            c.x = c.x + 1;
            c.y = c.y + 1;
            if self.tiles.get(&c) == expected {
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
        counter = 1;
        for i in 1..=min(4, min(position.x, position.y)) {
            c.x = c.x + 1;
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
        }
        if counter >= 5 {
            return true;
        }
        c = position.clone();
        for i in 1..=max(4, BOARD_SIZE - 1 - max(position.x, position.y)) {
            c.x = c.x - 1;
            c.y = c.y + 1;
            if self.tiles.get(&c) == expected {
                counter += 1;
            } else {
                break;
            }
            if counter >= 5 {
                return true;
            }
        }

        // 5. Check if board is filled -> Tie
        if self.tiles.len() >= (BOARD_SIZE * BOARD_SIZE) as u64 {
            return true;
        }
        false
    }

    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    pub fn update_winner(&mut self, coords: Coords) {
        if self.check_winner(coords) {
            if self.current_piece == Piece::X {
                self.winner = Some(Winner::X);
            } else if self.current_piece == Piece::O {
                self.winner = Some(Winner::O);
            } else {
                self.winner = Some(Winner::Tie);
            }
        }
    }
}
#[test]
fn valid_move() {
    // create two players
    let piece_1 = Piece::X;
    let piece_2 = Piece::O;
    let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
    let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

    // initialize the board
    let board = Board::new(&player_1, &player_2);

    // make move
    board.check_move(Coords { x: 0, y: 0 });
    assert_eq!(board.tiles.get(&Coords { x: 0, y: 0 }), Some(piece_1));
}
#[test]
#[should_panic(expected = "Provided position is invalid: row: 5 col: 5")]
fn index_out_of_bound() {
    // create two players
    let piece_1 = Piece::X;
    let piece_2 = Piece::O;
    let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
    let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

    // initialize the board
    let board = Board::new(&player_1, &player_2);

    // make move
    board.check_move(Coords { x: 0, y: 0 });
    assert_eq!(board.tiles.get(&Coords { x: 0, y: 0 }), Some(piece_1));
}
#[test]
#[should_panic(expected = "The tile row: 0 col: 0 already contained another piece: X")]
fn alreddy_taken_field() {
    // create two players
    let piece_1 = Piece::random();
    let piece_2 = piece_1.other();
    let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
    let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

    // initialize the board
    let board = Board::new(&player_1, &player_2);

    // make move
    board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_1);
    board.check_move(Coords { x: 0, y: 0 });
    assert_eq!(board.tiles.get(&Coords { x: 0, y: 0 }), Some(piece_1));
}
#[test]
fn check_winner() {
    // create two players
    let piece_1 = Piece::X;
    let piece_2 = Piece::O;
    let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
    let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

    // initialize the board
    let board = Board::new(&player_1, &player_2);

    // prepare the board
    // O O O O _
    // X X X X _
    // _ _ _ _ _
    // _ _ _ _ _
    // _ _ _ _ _
    board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_1);
    board.tiles.insert(&Coords { x: 1, y: 0 }, &piece_1);
    board.tiles.insert(&Coords { x: 2, y: 0 }, &piece_1);
    board.tiles.insert(&Coords { x: 3, y: 0 }, &piece_1);
    board.tiles.insert(&Coords { x: 0, y: 1 }, &piece_2);
    board.tiles.insert(&Coords { x: 1, y: 1 }, &piece_2);
    board.tiles.insert(&Coords { x: 2, y: 1 }, &piece_2);
    board.tiles.insert(&Coords { x: 3, y: 1 }, &piece_2);

    board.check_winner(Coords { x: 4, y: 0 });
    assert_eq!(board.check_winner(Coords { x: 4, y: 0 }), true);
    board.update_winner(Coords { x: 4, y: 0 });
    assert_eq!(board.winner, Some(Winner::X));
}
