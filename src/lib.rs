extern crate alloc;

use std::collections::HashMap;

use stylus_sdk::{alloy_primitives::*, prelude::*};

use alloc::vec;
use alloc::vec::Vec;

use libbucharesthashing::{immutables::*, prover, prover::Piece};

pub type Board = Vec<(u32, Piece, u32)>;

fn pos_to_xy(row_size: u32, p: u32) -> (u32, u32) {
    (p % row_size, p / row_size)
}

fn in_bounds(row_size: u32, x: u32, y: u32) -> bool {
    x < row_size && y < row_size
}

fn in_check_threats(board: &Board, row_size: u32, king_pos: u32) -> HashMap<u32, u32> {
    let mut map: HashMap<u32, u32> = HashMap::with_capacity(CHECKS_NEEDED as usize * 2);
    for (nonce, piece, piece_pos) in board {
        if is_checking(row_size, king_pos, *piece_pos, *piece) {
            map.insert(*piece_pos, *nonce);
        } else {
            if map.contains_key(piece_pos) {
                map.remove(piece_pos);
            }
        }
    }
    map
}

fn is_checking(row_size: u32, king_pos: u32, piece_pos: u32, piece: Piece) -> bool {
    let (king_x, king_y) = pos_to_xy(row_size, king_pos);
    let (piece_x, piece_y) = pos_to_xy(row_size, piece_pos);

    let dx = king_x.abs_diff(piece_x);
    let dy = king_y.abs_diff(piece_y);

    if dx == 0 && dy == 0 {
        return false;
    }

    match piece {
        Piece::PAWN => piece_y != u32::MAX && piece_y + 1 == king_y && dx == 1,
        Piece::CASTLE => dx == 0 || dy == 0,
        Piece::QUEEN => dx == 0 || dy == 0 || dx == dy,
        Piece::BISHOP => dx == dy,
        Piece::KNIGHT => dx * dy == 2,
        Piece::KING => dx <= 1 && dy <= 1,
    }
}

pub fn solve(starting_hash: &[u8], start: u32) -> Option<(u32, u32)> {
    println!("Starting solve");
    let row_size = BOARD_SIZE.isqrt();
    let mut board: Board = Vec::new();
    let mut last_king: Option<(u32, u32)> = None;
    let mut threat_map: HashMap<u32, u32> = HashMap::with_capacity(CHECKS_NEEDED as usize * 2);

    for i in start..MAX_TRIES {
        let e = prover::hash(starting_hash, i);
        let king_id: u8 = Piece::KING.into();
        let p_id: u8 = (e % (king_id as u64 + 1)).try_into().unwrap();
        let p = Piece::try_from(p_id).unwrap();
        let offset: u32 = (e >> 32).try_into().unwrap();
        let pos: u32 = offset % BOARD_SIZE;

        let (x, y) = pos_to_xy(row_size, pos);
        if !in_bounds(row_size, x, y) {
            continue;
        }

        board.push((i, p, pos));

        if p == Piece::KING {
            last_king = Some((pos, i));
            threat_map = in_check_threats(&board, row_size, pos);
        } else if let Some((last_king_pos, _)) = last_king {
            if is_checking(row_size, last_king_pos, pos, p) {
                threat_map.insert(pos, i);
            } else if threat_map.contains_key(&pos) {
                threat_map.remove(&pos);
            }
        }

        if let Some((_, last_king_nonce)) = last_king {
            if threat_map.len() >= CHECKS_NEEDED as usize {
                let min_threat = threat_map
                    .values()
                    .copied()
                    .chain(std::iter::once(last_king_nonce))
                    .min()
                    .unwrap();

                return Some((min_threat, i));
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
        #![proptest_config(ProptestConfig { cases: 5000, ..Default::default() })]
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
            let (p_l, p_h) = prover::default_solve(&starting_hash, 0).unwrap();
            assert_eq!(
                (e_l, e_h), (p_l, p_h),
                "user contract inconsistent with reference. {e_l} != {p_l} or {e_h} != {p_h}"
            );
        }
    }
}
