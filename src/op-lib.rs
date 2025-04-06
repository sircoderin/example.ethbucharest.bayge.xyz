extern crate alloc;

use std::collections::HashMap;

use stylus_sdk::{alloy_primitives::*, prelude::*};

use alloc::vec;
use alloc::vec::Vec;

mod prover_custom;

use libbucharesthashing::{immutables::*, prover, prover::Piece};

pub type Board = Vec<(u32, Piece, u32)>;

fn pos_to_xy(row_size: u32, p: u32) -> (u32, u32) {
    (p % row_size, p / row_size)
}

fn in_check_threats(board: &Board, row_size: u32, king_pos: u32) -> (Vec<u32>, HashMap<u32, u32>) {
    println!("KING POS IN CHECK THREATS {:?}", king_pos);
    let mut threats = vec![];
    let mut map: HashMap<u32, u32> = HashMap::new();
    for (nonce, piece, piece_pos) in board {
        if map.contains_key(piece_pos) {
            println!("Removing threat in_check_threats: {:?}", piece_pos);
            threats.retain(|&x| x != *map.get(piece_pos).unwrap());
            map.remove(piece_pos);
        }

        if is_checking(row_size, king_pos, *piece_pos, *piece) {
            println!(
                "Adding threat in_check_threats: piece {:?} {:?} with nonce {:?} when king at {:?}",
                piece,
                pos_to_xy(row_size, *piece_pos),
                *nonce,
                pos_to_xy(row_size, king_pos)
            );
            threats.push(*nonce);
            map.insert(*piece_pos, *nonce);
        }
    }
    (threats, map)
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
    let row_size = BOARD_SIZE.isqrt();
    let mut board: Board = Vec::new();
    let mut last_king: Option<(u32, u32)> = None;
    let mut threats: Vec<u32> = vec![];
    let mut map: HashMap<u32, u32> = HashMap::new();

    for i in start..MAX_TRIES {
        let e = prover::hash(starting_hash, i);
        let king_id: u8 = Piece::KING.into();
        let p_id: u8 = (e % (king_id as u64 + 1)).try_into().unwrap();
        let p = Piece::try_from(p_id).unwrap();
        let offset: u32 = (e >> 32).try_into().unwrap();
        let pos: u32 = offset % BOARD_SIZE;

        board.push((i, p, pos));

        if p == Piece::KING {
            println!("King changed to {:?}", pos);
            last_king = Some((pos, i));
            (threats, map) = in_check_threats(&board, row_size, pos);
            // if threats.len() > 0 || map.len() > 0 {
            //     println!("{:?} {:?}", threats, map);
            // }
        } else if let Some((last_king_pos, _)) = last_king {
            if map.contains_key(&pos) {
                threats.retain(|&x| x != *map.get(&pos).unwrap());
                map.remove(&pos);

                // println!("Removing threat: {:?}", pos);
            }

            if is_checking(row_size, last_king_pos, pos, p) {
                threats.push(i);
                map.insert(pos, i);
                // println!("Adding threat: {:?} on position {:?}", i, pos);
            }
        }

        println!("{:?} {:?}", pos, p);

        if let Some((_, last_king_nonce)) = last_king {
            if threats.len() >= CHECKS_NEEDED as usize {
                if !threats.contains(&last_king_nonce) {
                    threats.push(last_king_nonce);
                }

                let min_threat = *threats.iter().min().unwrap();
                println!("solution {:?} {:?}", min_threat, i);
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

// #[cfg(all(test, not(target_arch = "wasm32")))]
// mod test {
//     use super::*;
//     use proptest::prelude::*;

//     proptest! {
//         #![proptest_config(ProptestConfig { cases: 1, max_shrink_iters: 0, ..Default::default() })]
//         #[test]
//         fn test_solve(starting_hash in any::<[u8; 64]>()) {
//             println!("Try");
//             // First, let's test if the user-defined algorithm is consistent.
//             let (e_l, e_h) = solve(&starting_hash, 0).unwrap();
//             // Let's run our function against the first invocation of the function!
//             // let (t_l, t_h) = solve(&starting_hash, e_l).unwrap();
//             // Now let's check if it's consistent.
//             // assert_eq!((e_l, e_h), (t_l, t_h), "user contract not consistent. {e_l} != {t_l} or {e_h} != {t_h}");
//             // Now, let's test if the remote contract's prove function is consistent with the
//             // local function here.
//             let (p_l, p_h) = prover_custom::default_solve(&starting_hash, 0).unwrap();
//             assert_eq!(
//                 (e_l, e_h), (p_l, p_h),
//                 "user contract inconsistent with reference. {e_l} != {p_l} or {e_h} != {p_h}"
//             );
//         }
//     }
// }
