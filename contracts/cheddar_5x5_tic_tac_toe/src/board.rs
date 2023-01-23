use std::cmp::{max, min};

use crate::*;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::serde::{Deserialize, Serialize};

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Copy)]
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
    InvalidPosition { row: u8, col: u8 },
    /// The tile already contained another piece
    TileFilled {
        other_piece: Piece,
        row: u8,
        col: u8,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct Coords {
    pub x: u8,
    pub y: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct Board {
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
        // check rows
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
        //TODO: check vertical
        c = position.clone();
        counter = 1;

        //TODO: check diagonal (NW - SE)
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

        //TODO: check diagonal (NE - SW)
        c = position.clone();
        counter = 1;
        for i in 1..=min(4, min(position.x, position.y)) {}
    }

    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    pub fn update_winner(&mut self, row: usize, col: usize) {
        if self.check_winner(Coords { x: col, y: row }) {
            //TODO: update winner
        }

        //check collumns

        //TODO: change the implementation so the board can be any size. Especially the way we check the winner is not ideal for it.
        let tiles_row = [
            self.tiles.get(&Coords { x: row, y: 0 }),
            self.tiles.get(&Coords { x: row, y: 1 }),
            self.tiles.get(&Coords { x: row, y: 2 }),
            self.tiles.get(&Coords { x: row, y: 3 }),
            self.tiles.get(&Coords { x: row, y: 4 }),
        ];
        let tiles_col = [
            self.tiles.get(&Coords { x: 0, y: col }),
            self.tiles.get(&Coords { x: 1, y: col }),
            self.tiles.get(&Coords { x: 2, y: col }),
            self.tiles.get(&Coords { x: 3, y: col }),
            self.tiles.get(&Coords { x: 4, y: col }),
        ];

        // Diagonals (row, col)
        // 1. (0, 0), (1, 1), (2, 2), (3, 3), (4, 4)
        // 2. (0, 4), (1, 3), (2, 2), (3, 1), (4, 0)

        // Define diagonals
        let tiles_diagonal_1 = if row == col {
            // Diagonal 1
            [
                self.tiles.get(&Coords { x: 0, y: 0 }),
                self.tiles.get(&Coords { x: 1, y: 1 }),
                self.tiles.get(&Coords { x: 2, y: 2 }),
                self.tiles.get(&Coords { x: 3, y: 3 }),
                self.tiles.get(&Coords { x: 4, y: 4 }),
            ]
        } else {
            // This will never produce a winner, so it is suitable to use for the case where the
            // last move isn't on diagonal 1 anyway.
            [None, None, None, None, None]
        };

        // let tiles_diagonal_2 = if (rows - row - 1) == col {
        //     // Diagonal 2
        //     [
        //         self.tiles.get(&Coords { x: 0, y: 4 }),
        //         self.tiles.get(&Coords { x: 1, y: 3 }),
        //         self.tiles.get(&Coords { x: 2, y: 2 }),
        //         self.tiles.get(&Coords { x: 3, y: 1 }),
        //         self.tiles.get(&Coords { x: 4, y: 0 }),
        //     ]
        // } else {
        //     // Our last move isn't on diagonal 2.
        //     [None, None, None, None, None]
        // };

        // check given tiles (row, col, diagonal)
        fn check_winner(row: &[Option<Piece>]) -> Option<Winner> {
            if row[0] == row[1] && row[1] == row[2] && row[2] == row[3] && row[3] == row[4] {
                match row[0] {
                    Some(Piece::X) => Some(Winner::X),
                    Some(Piece::O) => Some(Winner::O),
                    None => None,
                }
            } else {
                None
            }
        }

        // Check winner for all given diagonals and rows/columns
        self.winner = self
            .winner
            .or_else(|| check_winner(&tiles_row))
            .or_else(|| check_winner(&tiles_col))
            .or_else(|| check_winner(&tiles_diagonal_1));       
            // .or_else(|| check_winner(&tiles_diagonal_2));

        // Tie case
        //TODO: this needs to be updated to work with the map
        self.winner = self.winner.or_else(|| {
            if self
                .tiles
                .iter()
                .all(|row| row.iter().all(|tile| tile.is_some()))
            {
                Some(Winner::Tie)
            } else {
                None
            }
        });
    }
}
//TODO: write a simple checks for functions that validates the move and check for the winner
// #[test]
// fn valid_move() {
//     // create two players
//     let piece_1 = Piece::random();
//     let piece_2 = piece_1.other();
//     let player_1 = Player::new(piece_1, account_id_1);
//     let player_2 = Player::new(piece_2, account_id_2);

//     // initialize the board
//     let board = Board::new(&player_1, &player_2);
//     board.check_move(0,0);
//     assert_eq!(board.tiles.get(&Coords{ x: 0, y: 0 }), piece_1.is_some());
// }
// #[test]
// #[should_panic(expected = "Provided position is invalid: row: 5 col: 5")]
// fn invalid_move() {
//     // create two players
//     let piece_1 = Piece::random();
//     let piece_2 = piece_1.other();
//     let player_1 = Player::new(piece_1, account_id_1);
//     let player_2 = Player::new(piece_2, account_id_2);

//     // initialize the board
//     let board = Board::new(&player_1, &player_2);
//     board.check_move(5,5);
// }
