use std::cmp::{max, min};

use crate::*;

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
    pub tiles: UnorderedMap<Coords, Piece>,
    /// X or O: who is currently playing
    pub current_piece: Piece,
    pub winner: Option<Winner>,
}

impl Board {
    /// there is two players with different AccountId's and
    /// with different random given pieces
    /// accounts check in `Game.create_game`
    pub fn new(game_id: GameId, player_1: &Player, player_2: &Player) -> Self {
        assert_ne!(
            player_1.piece, player_2.piece,
            "players have same pieces: {:?}",
            player_1.piece
        );
        Self {
            tiles: UnorderedMap::new(StorageKey::GameBoard { game_id }),
            current_piece: player_1.piece,
            winner: None,
        }
    }
    pub fn check_move(&self, row: usize, col: usize) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameOver);
        }
        if row >= BOARD_SIZE || col >= BOARD_SIZE {
            return Err(MoveError::InvalidPosition { row, col });
        }
        // Move in already filled tile
        else if let Some(other_piece) = self.tiles.get(&Coords {
            x: col as u8,
            y: row as u8,
        }) {
            return Err(MoveError::TileFilled {
                other_piece,
                row,
                col,
            });
        }
        Ok(())
    }
    pub fn check_winner(&self, position: Coords) -> bool {
        let expected = Some(self.current_piece.other());
        let mut c: Coords = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
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
        c = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
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
        // 2. check collumns
        c = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
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
        c = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
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
        c = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
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
        c = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
        for _ in 1..=max(
            4,
            BOARD_SIZE - 1 - max(position.x as usize, position.y as usize),
        ) {
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
        c = Coords {
            x: position.x.clone(),
            y: position.y.clone(),
        };
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
        c = Coords { x: position.x.clone(), y: position.y.clone() };
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
    pub fn update_winner(&mut self, coords: Coords) {
        if self.tiles.len() >= (BOARD_SIZE * BOARD_SIZE) as u64 {
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

    pub fn get_vector(&self) -> [[Option<Piece>; BOARD_SIZE as usize]; BOARD_SIZE as usize] {
        let mut local_vector: [[Option<Piece>; BOARD_SIZE as usize]; BOARD_SIZE as usize] =
            Default::default();

        // TODO:
        // + don't save matrix in the state. Intstead save two lists:
        //   (list of coords taken by X, list of coords taken by O)
        // + same for the UI query / view

        // let mut o_coords = Vec::new();
        // let mut x_coords = Vec::new();
        // for (c, p) in self.tiles.iter() {
        //     match p {
        //         Piece::O => o_coords.append(c),
        //         Piece::X => x_coords.append(c),
        //     }
        // }
        // return (o_coords, x_coords)

        for x in 0..=BOARD_SIZE - 1 {
            for y in 0..=BOARD_SIZE - 1 {
                local_vector[y as usize][x as usize] = self.tiles.get(&Coords {
                    x: x as u8,
                    y: y as u8,
                });
            }
        }
        return local_vector;
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use near_sdk::AccountId;

    use super::Board;
    use super::MoveError;
    use crate::{
        board::Coords,
        player::{Piece, Player},
        utils::BOARD_SIZE,
    };

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
        let _ = board.check_move(0, 0);
    }
    #[test]
    fn index_out_of_bound() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let board = Board::new(&player_1, &player_2);

        // make move
        let result = board.check_move(BOARD_SIZE, BOARD_SIZE);
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
        // create two players
        let piece_1 = Piece::random();
        let piece_2 = piece_1.other();
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

        // make move
        board.tiles.insert(&Coords { x: 0, y: 0 }, &piece_1);
        let result = board.check_move(0, 0);
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
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

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
        let result = board.check_winner(Coords { x: 4, y: 1 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_1() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

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
        let result = board.check_winner(Coords { x: 0, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_2() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

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
        let result = board.check_winner(Coords { x: 0, y: 0 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_column_winner_3() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

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
        let result = board.check_winner(Coords { x: 0, y: 2 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_1() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

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
        let result = board.check_winner(Coords { x: 4, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_se_diagonal_winner_2() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

        // prepare the board
        // _  _ _ _ _
        // _ O _ _ _
        // _ _ O _ _
        // _ _ _ O _
        // _ _ _ _ O
        board.tiles.insert(&Coords { x: 4, y: 4}, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2}, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 3}, &piece_2);
        let result = board.check_winner(Coords{ x: 0, y: 0 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_se_diagonal_winner_3() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

        // prepare the board
        // O _ _ _ _
        // _ O _ _ _
        // _ _ _ _ _
        // _ _ _ O _
        // _ _ _ _ O
        board.tiles.insert(&Coords { x: 0, y: 0}, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 4, y: 4}, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 3}, &piece_2);
        let result = board.check_winner(Coords{ x: 2, y: 2 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_sw_diagonal_winner_1() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

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
        let result = board.check_winner(Coords { x: 0, y: 4 });
        assert_eq!(result, true);
    }
    #[test]
    fn check_sw_diagonal_winner_2() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

        // prepare the board
        // _ _ _ _ _
        // _ _ _ O _
        // _ _ O _ _
        // _ O _ _ _
        // O _ _ _ _
        board.tiles.insert(&Coords { x: 0, y: 4}, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 3}, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2}, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1}, &piece_2);
        let result = board.check_winner(Coords{ x: 4, y: 0 });
        assert_eq!(result, true); 
    }
    #[test]
    fn check_sw_diagonal_winner_3() {
        // create two players
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

        // prepare the board
        // _ _ _ _ O
        // _ _ _ O _
        // _ _ O _ _
        // _ _ _ _ _
        // O _ _ _ _
        board.tiles.insert(&Coords { x: 4, y: 0}, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2}, &piece_2);
        board.tiles.insert(&Coords { x: 0, y: 4}, &piece_2);
        let result = board.check_winner(Coords{ x: 1, y: 3 });
        assert_eq!(result, true); 
    }
    #[test]
    fn test_get_vector() {
        let piece_1 = Piece::X;
        let piece_2 = Piece::O;
        let player_1 = Player::new(piece_1, AccountId::new_unchecked("test1".into()));
        let player_2 = Player::new(piece_2, AccountId::new_unchecked("test2".into()));

        // initialize the board
        let mut board = Board::new(&player_1, &player_2);

        // insert few testing values
        // _ _ _ _ O
        // _ _ _ O _
        // _ _ O _ _
        // _ O _ _ _
        // _ _ _ _ _
        board.tiles.insert(&Coords { x: 4, y: 0 }, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1 }, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 2 }, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 3 }, &piece_2);
        let vector = board.get_vector();
        assert_eq!(vector.len(), BOARD_SIZE);
        assert_eq!(vector[0].len(), BOARD_SIZE);
        assert_eq!(vector[0][4], Some(piece_2));
        assert_eq!(vector[1][3], Some(piece_2));
        assert_eq!(vector[2][2], Some(piece_2));
        assert_eq!(vector[3][1], Some(piece_2));
        assert_eq!(vector[0][0], None);
        assert_eq!(vector[4][4], None);
    }
}

