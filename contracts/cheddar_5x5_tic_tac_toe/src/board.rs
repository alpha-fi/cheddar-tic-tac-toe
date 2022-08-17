use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Clone, Copy)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub enum Winner {
    X,
    O,
    Tie
}

#[derive(Debug, Clone)]
pub enum MoveError {
    /// The game was already over when a move was attempted
    GameAlreadyOver,
    /// The position provided was invalid
    InvalidPosition { row: usize, col: usize },
    /// The tile already contained another piece
    TileFilled { other_piece: Piece, row: usize, col: usize },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct Board {
    pub(crate) tiles: [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE],
    // If game is active there is always current piece
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
            "players have same pieces: {:?}", player_1.piece
        );
        Self { 
            tiles: Default::default(), 
            current_piece: player_1.piece, 
            winner: None 
        }
    }
    pub fn check_move(&self, row: usize, col: usize) -> Result<(), MoveError> {
        if self.winner.is_some() {
            return Err(MoveError::GameAlreadyOver);
        }
        if row >= self.tiles.len() || col >= self.tiles[0].len() {
            return Err(MoveError::InvalidPosition {row, col});
        }
        // Move in already filled tile
        else if let Some(other_piece) = self.tiles[row][col] {
            return Err(MoveError::TileFilled {other_piece, row, col});
        }
        Ok(())
    }
    /// To find a potential winner, we only need to check the row, column and (maybe) diagonal
    /// that the last move was made in.
    pub fn update_winner(&mut self, row: usize, col: usize) {

        let rows = self.tiles.len();
        let cols = self.tiles[0].len();

        let tiles_row = self.tiles[row];
        let tiles_col = [
            self.tiles[0][col], 
            self.tiles[1][col], 
            self.tiles[2][col],
            self.tiles[3][col],
            self.tiles[4][col],
        ];

        assert!(rows == BOARD_SIZE && cols == BOARD_SIZE);

        // Diagonals (row, col)
        // 1. (0, 0), (1, 1), (2, 2), (3, 3), (4, 4)
        // 2. (0, 4), (1, 3), (2, 2), (3, 1), (4, 0) 

        // Define diagonals
        let tiles_diagonal_1 = if row == col {
            // Diagonal 1
            [self.tiles[0][0], self.tiles[1][1], self.tiles[2][2], self.tiles[3][3], self.tiles[4][4]]
        }
        else {
            // This will never produce a winner, so it is suitable to use for the case where the
            // last move isn't on diagonal 1 anyway.
            [None, None, None, None, None]
        };

        let tiles_diagonal_2 = if (rows - row - 1) == col {
            // Diagonal 2
            [self.tiles[0][4], self.tiles[1][3], self.tiles[2][2], self.tiles[3][1], self.tiles[4][0]]
        }
        else {
            // Our last move isn't on diagonal 2.
            [None, None, None, None, None]
        };

        // check given tiles (row, col, diagonal)
        fn check_winner(row: &[Option<Piece>]) -> Option<Winner> {
            if row[0] == row[1] && row[1] == row[2] && row[2] == row[3] && row[3] == row[4]{
                match row[0] {
                    Some(Piece::X) => Some(Winner::X),
                    Some(Piece::O) => Some(Winner::O),
                    None => None,
                }
            }
            else {
                None
            }
        }

        // Check winner for all given diagonals and rows/columns
        self.winner = self.winner
            .or_else(|| check_winner(&tiles_row))
            .or_else(|| check_winner(&tiles_col))
            .or_else(|| check_winner(&tiles_diagonal_1))
            .or_else(|| check_winner(&tiles_diagonal_2));

        // Tie case
        self.winner = self.winner.or_else(|| {
            if self.tiles.iter().all(|row| row.iter().all(|tile| tile.is_some())) {
                Some(Winner::Tie)
            }
            else {
                None
            }
        });
    }
}