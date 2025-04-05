extern crate alloc;

use stylus_sdk::{alloy_primitives::*, prelude::*};

use alloc::vec;
use alloc::vec::Vec;

use libbucharesthashing::{immutables::*, prover, prover::Piece};

pub type Board = Vec<(u32, Piece, u32)>;

fn pos_to_xy(row_size: u32, p: u32) -> (u32, u32) {
    (p % row_size, p / row_size)
}

fn in_check_threats(board: &Board, row_size: u32, king_pos: u32) -> Vec<u32> {
    let mut threats = vec![];
    for (nonce, piece, piece_pos) in board {
        if is_solved(row_size, king_pos, *piece_pos, *piece) {
            threats.push(*nonce);
        }
    }
    threats
}

fn is_solved(row_size: u32, king_pos: u32, piece_pos: u32, piece: Piece) -> bool {
    let (king_x, king_y) = pos_to_xy(row_size, king_pos);
    let (piece_x, piece_y) = pos_to_xy(row_size, piece_pos);

    let dx = king_x.abs_diff(piece_x);
    let dy = king_y.abs_diff(piece_y);

    if dx == 0 && dy == 0 {
        return false;
    }

    if piece_y == u32::MAX {
        return false;
    }

    match piece {
        Piece::PAWN => piece_y + 1 == king_y && dx == 1,
        Piece::CASTLE => dx == 0 || dy == 0,
        Piece::QUEEN => dx == 0 || dy == 0 || dx == dy,
        Piece::BISHOP => dx == dy,
        Piece::KNIGHT => dx * dy == 2,
        Piece::KING => dx <= 1 && dy <= 1,
    }
}

pub fn solve(starting_hash: &[u8], start: u32) -> Option<(u32, u32)> {
    let row_size = BOARD_SIZE.isqrt();
    let mut board: Board = Vec::new();
    let mut last_king: Option<(u32, u32)> = None;
    let mut threats: Vec<u32> = vec![];

    for i in start..MAX_TRIES {
        let e = prover::hash(starting_hash, i);
        let king_id: u8 = Piece::KING.into();
        let p_id: u8 = (e % (king_id as u64 + 1)).try_into().unwrap();
        let p = Piece::try_from(p_id).unwrap();
        let offset: u32 = (e >> 32).try_into().unwrap();
        let pos: u32 = offset % BOARD_SIZE;

        if p != Piece::KING {
            for (index, (_, _, piece_pos)) in board.iter().enumerate() {
                if *piece_pos == pos {
                    board.remove(index);
                    break;
                }
            }
        }

        board.push((i, p, pos));

        if p == Piece::KING {
            last_king = Some((pos, i));
            threats = in_check_threats(&board, row_size, pos);
        } else if let Some((last_king_pos, _)) = last_king {
            if is_solved(row_size, last_king_pos, pos, p) {
                threats.push(i);
            }
        }

        if threats.len() >= CHECKS_NEEDED as usize {
            if let Some(first_threat) = threats.iter().min() {
                return Some((*first_threat, i));
            }
        }
    }
    None
}

#[storage]
#[entrypoint]
pub struct Storage {}

#[public]
impl Storage {
    pub fn prove(&self, hash: FixedBytes<32>, from: u32) -> Result<(u32, u32), Vec<u8>> {
        match solve(hash.as_slice(), from) {
            Some(result) => Ok(result),
            None => Err(vec![]),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_solve(starting_hash in any::<[u8; 64]>()) {
            // First, let's test if the user-defined algorithm is consistent.
            let (e_l, e_h) = solve(&starting_hash, 0).unwrap();
            // Let's run our function against the first invocation of the function!
            let (t_l, t_h) = solve(&starting_hash, e_l).unwrap();
            // Now let's check if it's consistent.
            assert_eq!((e_l, e_h), (t_l, t_h), "user contract not consistent. {e_l} != {t_l} or {e_h} != {t_h}");
            // Now, let's test if the remote contract's prove function is consistent with the
            // local function here.
            let (p_l, p_h) = prover::default_solve(&starting_hash, e_l).unwrap();
            assert_eq!(
                (e_l, e_h), (p_l, p_h),
                "user contract inconsistent with reference. {e_l} != {p_l} or {e_h} != {p_h}"
            );
        }
    }
}
