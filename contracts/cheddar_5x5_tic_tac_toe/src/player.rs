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