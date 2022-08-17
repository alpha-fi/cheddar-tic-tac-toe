use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Clone, Copy, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum Piece {
    X,
    O,
}

impl Piece {
    pub fn other(self) -> Piece {
        match self {
            Piece::X => Piece::O,
            Piece::O => Piece::X,
        }
    }
    pub fn random() -> Piece {
        let seed = near_sdk::env::random_seed();
        match seed[0] % 2 {
            0 => Piece::X,
            _ => Piece::O
        }
    }
}

/// Player struct with X/O and `AccountId`
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct Player {
	pub piece : Piece,
    pub account_id: AccountId
}

impl Player {
    pub fn new(piece: Piece, account_id: AccountId) -> Self {
        Self { 
            piece, 
            account_id
        }
    }
}