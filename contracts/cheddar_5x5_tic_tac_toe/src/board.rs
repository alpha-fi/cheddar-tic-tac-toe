use std::cmp::{max, min};

use crate::{views::Tiles, *};

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

#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct Board {
    pub tiles: UnorderedMap<Coords, Piece>,
    pub last_move: Option<Coords>,
    /// X or O: who is currently playing
    pub current_piece: Piece,
    pub winner: Option<Winner>,
}

impl Board {
    /// there is two players with different AccountId's and
    /// with different random given pieces
    /// accounts check in `Game.create_game`
    pub fn new(game_id: GameId) -> Self {
        Self {
            tiles: UnorderedMap::new(StorageKey::GameBoard { game_id }),
            current_piece: Piece::O,
            winner: None,
            last_move: None,
        }
    }
    pub fn check_move(&self, coords: &Coords) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameOver);
        }
        if coords.y >= BOARD_SIZE as u8 || coords.x >= BOARD_SIZE as u8 {
            return Err(MoveError::InvalidPosition {
                row: coords.y,
                col: coords.x,
            });
        }
        // Move in already filled tile
        else if let Some(other_piece) = self.tiles.get(&coords) {
            return Err(MoveError::TileFilled {
                other_piece,
                row: coords.y,
                col: coords.x,
            });
        }
        Ok(())
    }
    pub fn check_winner(&mut self, position: &Coords) -> bool {
        let expected = Some(self.current_piece.other());
        self.current_piece = self.current_piece.other();
        let mut c = position.clone();
        let mut counter = 1;
        // 1. check rows
        // go max 4 pos to the left and see how far we can go
        for _ in 1..=min(4, position.x) {
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - position.x as usize) {
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
        // 2. check columns
        c = position.clone();
        counter = 1;
        for _ in 1..=min(4, position.y) {
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - position.y as usize) {
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
        for _ in 1..=min(4, min(position.x, position.y)) {
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - max(position.x, position.y) as usize) {
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
        let mut counter = 1;
        for _ in 1..=position.y {
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
        for _ in 1..=position.x {
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
        false
    }
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    pub fn update_winner(&mut self, coords: &Coords) {
        if self.tiles.len() >= MAX_NUM_TURNS {
            self.winner = Some(Winner::Tie);
            return;
        }
        if self.check_winner(coords) {
            if self.current_piece == Piece::X {
                self.winner = Some(Winner::X);
                print!("X is the winner");
            } else if self.current_piece == Piece::O {
                self.winner = Some(Winner::O);
                print!("O is the winner");
            }
        }
    }

    pub fn to_tiles(&self) -> Tiles {
        let mut o_coords = Vec::new();
        let mut x_coords = Vec::new();
        for (c, p) in self.tiles.iter() {
            match p {
                Piece::O => o_coords.push(c),
                Piece::X => x_coords.push(c),
            }
        }
        return Tiles { o_coords, x_coords };
    }
    pub fn get_last_move(&self) -> Coords {
        return self.last_move.clone().unwrap();
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
    use super::Board;
    use super::MoveError;
    use crate::{board::Coords, player::Piece, utils::BOARD_SIZE};

    #[test]
    fn valid_move() {
        let game_id: u64 = 1;
        // initialize the board
        let board = Board::new(game_id);

        // make move
        let _ = board.check_move(&Coords { x: 0, y: 0 });
    }
    #[test]
    fn index_out_of_bound() {
        let game_id: u64 = 1;
        // initialize the board
        let board = Board::new(game_id);

        // make move
        let result = board.check_move(&Coords {
            x: BOARD_SIZE as u8,
            y: BOARD_SIZE as u8,
        });
        assert_eq!(
            result,
            Err(MoveError::InvalidPosition {
                row: BOARD_SIZE as u8,
                col: BOARD_SIZE as u8
            })
        );
    }
    #[test]
    fn alreddy_taken_field() {
        // create random Piece
        let piece_1 = Piece::random();
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // make move
        board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_1);
        let result = board.check_move(&Coords { x: 0, y: 0 });
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
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

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
        let result = board.check_winner(&Coords { x: 4, y: 1 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_1() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);
        // prepare the board
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // _ _ _ _ _
        board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 3 }, &piece_2);
        let result = board.check_winner(&Coords { x: 0, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_2() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // _ _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        board.tiles.insert(&Coords { x: 0, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 3 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 4 }, &piece_2);
        let result = board.check_winner(&Coords { x: 0, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_3() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // O _ _ _ _
        // O _ _ _ _
        // _ _ _ _ _
        // O _ _ _ _
        // O _ _ _ _
        board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 3 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 4 }, &piece_2);
        let result = board.check_winner(&Coords { x: 0, y: 2 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_1() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // O _ _ _ _
        // _ O _ _ _
        // _ _ O _ _
        // _ _ _ O _
        // _ _ _ _ _
        board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 3 }, &piece_2);
        let result = board.check_winner(&Coords { x: 4, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_2() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // _  _ _ _ _
        // _ O _ _ _
        // _ _ O _ _
        // _ _ _ O _
        // _ _ _ _ O
        board.tiles.insert(&Coords { x: 4, y: 4 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 3 }, &piece_2);
        let result = board.check_winner(&Coords { x: 0, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_3() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // O _ _ _ _
        // _ O _ _ _
        // _ _ _ _ _
        // _ _ _ O _
        // _ _ _ _ O
        board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 4, y: 4 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 3 }, &piece_2);
        let result = board.check_winner(&Coords { x: 2, y: 2 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_1() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // _ _ _ _ O
        // _ _ _ O _
        // _ _ O _ _
        // _ O _ _ _
        // _ _ _ _ _
        board.tiles.insert(&Coords { x: 4, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 3 }, &piece_2);
        let result = board.check_winner(&Coords { x: 0, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_2() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // _ _ _ _ _
        // _ _ _ O _
        // _ _ O _ _
        // _ O _ _ _
        // O _ _ _ _
        board.tiles.insert(&Coords { x: 0, y: 4 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 3 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1 }, &piece_2);
        let result = board.check_winner(&Coords { x: 4, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_3() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        // _ _ _ _ O
        // _ _ _ O _
        // _ _ O _ _
        // _ _ _ _ _
        // O _ _ _ _
        board.tiles.insert(&Coords { x: 4, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 4 }, &piece_2);
        let result = board.check_winner(&Coords { x: 1, y: 3 });
        assert_eq!(result, true);
    }
    #[test]
    fn test_board_25_x_25() {
        let game_id: u64 = 1;
        // initialize the board
        let board = Board::new(game_id);

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
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        board.tiles.insert(&Coords { x: 24, y: 0}, &piece_2);
        board.tiles.insert(&Coords { x: 23, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 22, y: 2}, &piece_2);
        board.tiles.insert(&Coords { x: 20, y: 4}, &piece_2);
        let result = board.check_winner(&Coords{ x: 21, y: 3 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_row_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        board.tiles.insert(&Coords { x: 24, y: 20}, &piece_2);
        board.tiles.insert(&Coords { x: 23, y: 20}, &piece_2);
        board.tiles.insert(&Coords { x: 22, y: 20}, &piece_2);
        board.tiles.insert(&Coords { x: 21, y: 20}, &piece_2);
        let result = board.check_winner(&Coords{ x: 20, y: 20 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_diagonal_se_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        board.tiles.insert(&Coords { x: 24, y: 24}, &piece_2);
        board.tiles.insert(&Coords { x: 23, y: 23}, &piece_2);
        board.tiles.insert(&Coords { x: 21, y: 21}, &piece_2);
        board.tiles.insert(&Coords { x: 20, y: 20}, &piece_2);
        let result = board.check_winner(&Coords{ x: 22, y: 22 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_horizontal_bottom_edge_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // prepare the board
        board.tiles.insert(&Coords { x: 24, y: 20}, &piece_2);
        board.tiles.insert(&Coords { x: 24, y: 21}, &piece_2);
        board.tiles.insert(&Coords { x: 24, y: 22}, &piece_2);
        board.tiles.insert(&Coords { x: 24, y: 23}, &piece_2);
        let result = board.check_winner(&Coords{ x: 24, y: 24 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_horizontal_top_edge_winner_25x25() {
        // create two players
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);
        assert_eq!(board.current_piece, Piece::O);

        // prepare the board
        board.tiles.insert(&Coords { x: 24, y: 0}, &piece_2);
        board.tiles.insert(&Coords { x: 24, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 24, y: 2}, &piece_2);
        board.tiles.insert(&Coords { x: 24, y: 3}, &piece_2);
        let result = board.check_winner(&Coords{ x: 24, y: 4 });
        assert_eq!(board.current_piece, Piece::X);
        assert_eq!(result, true); 
    }
    #[test]
    fn test_to_tiles() {
        let piece_2 = Piece::X;
        let game_id: u64 = 1;
        // initialize the board
        let mut board = Board::new(game_id);

        // insert few testing values
        // _ _ _ _ X
        // _ _ _ X _
        // _ _ X _ _
        // _ X _ _ _
        // _ _ _ _ _
        board.tiles.insert(&Coords { x: 4, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 3 }, &piece_2);
        let vector = board.to_tiles();
        assert_eq!(vector.o_coords.len(), 0);
        assert_eq!(vector.x_coords.len(), 4);
        assert_eq!(vector.x_coords[0], Coords { x: 4, y: 0 });
        assert_eq!(vector.x_coords[1], Coords { x: 3, y: 1 });
        assert_eq!(vector.x_coords[2], Coords { x: 2, y: 2 });
        assert_eq!(vector.x_coords[3], Coords { x: 1, y: 3 });
    }
}
