use std::cmp::{min, max};

use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Copy)]
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
            tiles: UnorderedMap::new(env::sha256(&player_1.account_id.as_bytes())),//account hash
            current_piece: player_1.piece,
            winner: None,
        }
    }
    pub fn check_move(&self, row: usize, col: usize) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameAlreadyOver);
        }
        if row >= BOARD_SIZE || col >= BOARD_SIZE {
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
        let expected = Some(self.current_piece.other());
        let mut c: Coords = Coords { x: position.x.clone(), y: position.y.clone() };
        let mut counter = 1;

        // 1. check rows
        // go max 4 pos to the left and see how far we can go
        for _ in 1..=min(4, position.x) {
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - position.x as usize) {
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
        for _ in 1..=min(4, position.y) {
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - position.y as usize) {
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
        for _ in 1..=min(4, min(position.x, position.y)) {
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - max(position.x as usize, position.y as usize)) {
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
        let mut counter = 1;
        for _ in 1..=min(4, min(position.x, position.y)) {
            c.x = c.x - 1;
            c.y = c.y + 1;
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
        for _ in 1..=max(4, BOARD_SIZE - 1 - max(position.x as usize, position.y as usize)) {
            if c.y == 0 {
                break;
            }
            c.x = c.x + 1;
            c.y = c.y - 1;
            if self.tiles.get(&c) == expected {
                counter+=1;
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
        if self.tiles.len() >= (BOARD_SIZE * BOARD_SIZE) as u64{
            self.winner = Some(Winner::Tie);
            return;
        }
        if self.check_winner(coords) {
            if self.current_piece == Piece::X{
                self.winner =  Some(Winner::X);
                print!("X is the winner");
            } else if self.current_piece == Piece::O{
                self.winner =  Some(Winner::O);
                print!("O is the winner");
            }
        }
    }

    pub fn get_vector(&self) -> [[Option<Piece>; BOARD_SIZE as usize]; BOARD_SIZE as usize] {
        let mut local_vector: [[Option<Piece>; BOARD_SIZE as usize]; BOARD_SIZE as usize] = Default::default();
        for x in 0..=BOARD_SIZE - 1 {
            for y in 0..=BOARD_SIZE - 1 {
                local_vector[y as usize][x as usize] = self.tiles.get(&Coords { x: x as u8, y: y as u8 });
            }
        }
        return local_vector;
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use near_sdk::AccountId;

    use crate::{player::{Piece, Player}, utils::BOARD_SIZE, board::Coords};
    use super::MoveError;
    use super::Board;

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
        let result = board.check_move(BOARD_SIZE,BOARD_SIZE);
        assert_eq!(result, Err(MoveError::InvalidPosition { row: BOARD_SIZE, col: BOARD_SIZE }));
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
        board.tiles.insert(&Coords { x: 0, y: 0}, &piece_1);
        let result = board.check_move(0,0);
        assert_eq!(result, Err(MoveError::TileFilled {
            other_piece: piece_1,
            row: 0,
            col: 0})); 
    }
    #[test]
    fn check_winner() {
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
        board.tiles.insert(&Coords { x: 0, y: 0}, &piece_1);
        board.tiles.insert(&Coords { x: 1, y: 0}, &piece_1);
        board.tiles.insert(&Coords { x: 2, y: 0}, &piece_1);
        board.tiles.insert(&Coords { x: 3, y: 0}, &piece_1);
        board.tiles.insert(&Coords { x: 0, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 1, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 2, y: 1}, &piece_2);
        board.tiles.insert(&Coords { x: 3, y: 1}, &piece_2);
        let result = board.check_winner(Coords{ x: 4, y: 1 });
        assert_eq!(result, true); 
    }
}