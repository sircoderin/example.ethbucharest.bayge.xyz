extern crate alloc;

use stylus_sdk::{alloy_primitives::*, prelude::*};

use alloc::{collections::BTreeMap, vec, vec::Vec};

use libbucharesthashing::{immutables::*, prover, prover::Piece};

pub type Board = BTreeMap<u32, (Piece, u32)>;

const KNIGHT_MOVES: [(i32, i32); 8] = [
    (-2, -1), (-2, 1), (-1, -2), (-1, 2),
    (1, -2), (1, 2), (2, -1), (2, 1),
];

const KING_MOVES: [(i32, i32); 8] = [
    (-1, -1), (-1, 0), (-1, 1), (0, -1),
    (0, 1), (1, -1), (1, 0), (1, 1),
];

fn pos_to_xy(row_size: u32, p: u32) -> (u32, u32) {
    (p % row_size, p / row_size)
}

fn xy_to_pos(row_size: u32, x: u32, y: u32) -> u32 {
    y.wrapping_mul(row_size).wrapping_add(x)
}

fn in_bounds(row_size: u32, x: u32, y: u32) -> bool {
    x < row_size && y < row_size
}

fn get_diagonal(x: u32, y: u32) -> u32 {
    x.wrapping_add(y)
}

fn get_anti_diagonal(x: u32, y: u32, row_size: u32) -> u32 {
    if y <= x.wrapping_add(row_size).wrapping_sub(1) {
        x.wrapping_add(row_size).wrapping_sub(1).wrapping_sub(y)
    } else {
        row_size * 2
    }
}

fn get_y_on_anti_diagonal(x: u32, anti_diag: u32, row_size: u32) -> Option<u32> {
    if anti_diag <= x.wrapping_add(row_size).wrapping_sub(1) {
        let y = x.wrapping_add(row_size).wrapping_sub(1).wrapping_sub(anti_diag);
        if y < row_size {
            return Some(y);
        }
    }
    None
}

fn is_checking(row_size: u32, king_pos: u32, piece_pos: u32, piece: Piece) -> bool {
    if king_pos == piece_pos {
        return false;
    }

    let (king_x, king_y) = pos_to_xy(row_size, king_pos);
    let (piece_x, piece_y) = pos_to_xy(row_size, piece_pos);

    let dx = if king_x >= piece_x { king_x - piece_x } else { piece_x - king_x };
    let dy = if king_y >= piece_y { king_y - piece_y } else { piece_y - king_y };

    match piece {
        Piece::PAWN => piece_y + 1 == king_y && dx == 1,
        Piece::CASTLE => dx == 0 || dy == 0,
        Piece::QUEEN => dx == 0 || dy == 0 || dx == dy,
        Piece::BISHOP => dx == dy,
        Piece::KNIGHT => dx * dy == 2,
        Piece::KING => dx <= 1 && dy <= 1,
    }
}

struct ThreatTracker {
    rows: Vec<u32>,
    cols: Vec<u32>,
    diags: Vec<u32>,
    anti_diags: Vec<u32>,
    
    row_size: u32,
}

impl ThreatTracker {
    fn new(board_size: u32) -> Self {
        let row_size = board_size.isqrt();
        let diag_count = (2 * row_size - 1) as usize;
        
        ThreatTracker {
            rows: vec![0; row_size as usize],
            cols: vec![0; row_size as usize],
            diags: vec![0; diag_count],
            anti_diags: vec![0; diag_count],
            row_size,
        }
    }
    
    fn place_piece(&mut self, board: &mut Board, pos: u32, piece: Piece, nonce: u32) {
        let (x, y) = pos_to_xy(self.row_size, pos);
        
        if let Some((old_piece, _)) = board.get(&pos) {
            self.remove_piece_threats(pos, *old_piece);
        }
        
        board.insert(pos, (piece, nonce));
        
        self.add_piece_threats(pos, piece);
    }
    
    fn add_piece_threats(&mut self, pos: u32, piece: Piece) {
        let (x, y) = pos_to_xy(self.row_size, pos);
        
        if x >= self.row_size || y >= self.row_size {
            return;
        }
        
        match piece {
            Piece::CASTLE => {
                if y < self.rows.len() as u32 {
                    self.rows[y as usize] += 1;
                }
                if x < self.cols.len() as u32 {
                    self.cols[x as usize] += 1;
                }
            },
            Piece::BISHOP => {
                let diag = get_diagonal(x, y);
                let anti_diag = get_anti_diagonal(x, y, self.row_size);
                
                if diag < self.diags.len() as u32 {
                    self.diags[diag as usize] += 1;
                }
                if anti_diag < self.anti_diags.len() as u32 {
                    self.anti_diags[anti_diag as usize] += 1;
                }
            },
            Piece::QUEEN => {
                if y < self.rows.len() as u32 {
                    self.rows[y as usize] += 1;
                }
                if x < self.cols.len() as u32 {
                    self.cols[x as usize] += 1;
                }
                
                let diag = get_diagonal(x, y);
                let anti_diag = get_anti_diagonal(x, y, self.row_size);
                
                if diag < self.diags.len() as u32 {
                    self.diags[diag as usize] += 1;
                }
                if anti_diag < self.anti_diags.len() as u32 {
                    self.anti_diags[anti_diag as usize] += 1;
                }
            },
            _ => {}
        }
    }
    
    fn remove_piece_threats(&mut self, pos: u32, piece: Piece) {
        let (x, y) = pos_to_xy(self.row_size, pos);
        
        if x >= self.row_size || y >= self.row_size {
            return;
        }
        
        match piece {
            Piece::CASTLE => {
                if y < self.rows.len() as u32 {
                    self.rows[y as usize] = self.rows[y as usize].saturating_sub(1);
                }
                if x < self.cols.len() as u32 {
                    self.cols[x as usize] = self.cols[x as usize].saturating_sub(1);
                }
            },
            Piece::BISHOP => {
                let diag = get_diagonal(x, y);
                let anti_diag = get_anti_diagonal(x, y, self.row_size);
                
                if diag < self.diags.len() as u32 {
                    self.diags[diag as usize] = self.diags[diag as usize].saturating_sub(1);
                }
                if anti_diag < self.anti_diags.len() as u32 {
                    self.anti_diags[anti_diag as usize] = self.anti_diags[anti_diag as usize].saturating_sub(1);
                }
            },
            Piece::QUEEN => {
                if y < self.rows.len() as u32 {
                    self.rows[y as usize] = self.rows[y as usize].saturating_sub(1);
                }
                if x < self.cols.len() as u32 {
                    self.cols[x as usize] = self.cols[x as usize].saturating_sub(1);
                }
                
                let diag = get_diagonal(x, y);
                let anti_diag = get_anti_diagonal(x, y, self.row_size);
                
                if diag < self.diags.len() as u32 {
                    self.diags[diag as usize] = self.diags[diag as usize].saturating_sub(1);
                }
                if anti_diag < self.anti_diags.len() as u32 {
                    self.anti_diags[anti_diag as usize] = self.anti_diags[anti_diag as usize].saturating_sub(1);
                }
            },
            _ => {}
        }
    }
    
    fn calculate_threats(&self, board: &Board, king_pos: u32) -> Vec<u32> {
        let (king_x, king_y) = pos_to_xy(self.row_size, king_pos);
        let mut threats = Vec::new();
        
        if king_x >= self.row_size || king_y >= self.row_size {
            return threats;
        }
        
        self.add_point_attackers(board, king_x, king_y, &mut threats);
        
        self.add_sliding_piece_attackers(board, king_pos, king_x, king_y, &mut threats);
        
        threats
    }
    
    fn add_point_attackers(&self, board: &Board, king_x: u32, king_y: u32, threats: &mut Vec<u32>) {
        for dx in [-1i32, 1] {
            let x = king_x.wrapping_add_signed(dx);
            let y = king_y.wrapping_sub(1);
            
            if in_bounds(self.row_size, x, y) {
                let pos = xy_to_pos(self.row_size, x, y);
                if let Some((piece, nonce)) = board.get(&pos) {
                    if *piece == Piece::PAWN {
                        threats.push(*nonce);
                    }
                }
            }
        }
        
        for &(dx, dy) in &KNIGHT_MOVES {
            let x = king_x.wrapping_add_signed(dx);
            let y = king_y.wrapping_add_signed(dy);
            
            if in_bounds(self.row_size, x, y) {
                let pos = xy_to_pos(self.row_size, x, y);
                if let Some((piece, nonce)) = board.get(&pos) {
                    if *piece == Piece::KNIGHT {
                        threats.push(*nonce);
                    }
                }
            }
        }
        
        for &(dx, dy) in &KING_MOVES {
            let x = king_x.wrapping_add_signed(dx);
            let y = king_y.wrapping_add_signed(dy);
            
            if in_bounds(self.row_size, x, y) {
                let pos = xy_to_pos(self.row_size, x, y);
                if let Some((piece, nonce)) = board.get(&pos) {
                    if *piece == Piece::KING {
                        threats.push(*nonce);
                    }
                }
            }
        }
    }
    
    fn add_sliding_piece_attackers(&self, board: &Board, king_pos: u32, king_x: u32, king_y: u32, threats: &mut Vec<u32>) {
        if king_y < self.rows.len() as u32 && self.rows[king_y as usize] > 0 {
            for x in 0..self.row_size {
                if x != king_x {
                    let pos = xy_to_pos(self.row_size, x, king_y);
                    if let Some((piece, nonce)) = board.get(&pos) {
                        if *piece == Piece::CASTLE || *piece == Piece::QUEEN {
                            threats.push(*nonce);
                        }
                    }
                }
            }
        }
        
        if king_x < self.cols.len() as u32 && self.cols[king_x as usize] > 0 {
            for y in 0..self.row_size {
                if y != king_y {
                    let pos = xy_to_pos(self.row_size, king_x, y);
                    if let Some((piece, nonce)) = board.get(&pos) {
                        if *piece == Piece::CASTLE || *piece == Piece::QUEEN {
                            threats.push(*nonce);
                        }
                    }
                }
            }
        }
        
        let diag = get_diagonal(king_x, king_y);
        if diag < self.diags.len() as u32 && self.diags[diag as usize] > 0 {
            for x in 0..self.row_size {
                if x != king_x {
                    let y = diag.wrapping_sub(x);
                    if y < self.row_size {
                        let pos = xy_to_pos(self.row_size, x, y);
                        if let Some((piece, nonce)) = board.get(&pos) {
                            if *piece == Piece::BISHOP || *piece == Piece::QUEEN {
                                threats.push(*nonce);
                            }
                        }
                    }
                }
            }
        }
        
        let anti_diag = get_anti_diagonal(king_x, king_y, self.row_size);
        if anti_diag < self.anti_diags.len() as u32 && self.anti_diags[anti_diag as usize] > 0 {
            for x in 0..self.row_size {
                if x != king_x {
                    if let Some(y) = get_y_on_anti_diagonal(x, anti_diag, self.row_size) {
                        let pos = xy_to_pos(self.row_size, x, y);
                        if let Some((piece, nonce)) = board.get(&pos) {
                            if *piece == Piece::BISHOP || *piece == Piece::QUEEN {
                                threats.push(*nonce);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn solve(starting_hash: &[u8], start: u32) -> Option<(u32, u32)> {
    let row_size = BOARD_SIZE.isqrt();
    let mut board = BTreeMap::new();
    let mut threat_tracker = ThreatTracker::new(BOARD_SIZE);
    
    let mut last_king = None;
    let mut threats = Vec::new();
    
    for j in 0..start {
        let e = prover::hash(starting_hash, j);
        let king_id: u8 = Piece::KING.into();
        let p_id: u8 = (e % (king_id as u64 + 1)).try_into().unwrap();
        let p = Piece::try_from(p_id).unwrap();
        let offset: u32 = (e >> 32).try_into().unwrap();
        let pos: u32 = offset % BOARD_SIZE;
        
        threat_tracker.place_piece(&mut board, pos, p, j);
        
        if p == Piece::KING {
            last_king = Some((pos, j));
        }
    }
    
    if let Some((king_pos, _)) = last_king {
        threats = threat_tracker.calculate_threats(&board, king_pos);
    }
    
    for i in start..MAX_TRIES {
        let e = prover::hash(starting_hash, i);
        let king_id: u8 = Piece::KING.into();
        let p_id: u8 = (e % (king_id as u64 + 1)).try_into().unwrap();
        let p = Piece::try_from(p_id).unwrap();
        let offset: u32 = (e >> 32).try_into().unwrap();
        let pos: u32 = offset % BOARD_SIZE;
        
        threat_tracker.place_piece(&mut board, pos, p, i);
        
        if p == Piece::KING {
            last_king = Some((pos, i));
            threats = threat_tracker.calculate_threats(&board, pos);
        } 
        else if let Some((king_pos, _)) = last_king {
            if is_checking(row_size, king_pos, pos, p) {
                threats.push(i);
            }
        }
        
        if let Some((_, last_king_nonce)) = last_king {
            if threats.len() >= CHECKS_NEEDED as usize {
                if !threats.contains(&last_king_nonce) {
                    threats.push(last_king_nonce);
                }
                
                let min_threat = *threats.iter().min().unwrap();
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
            None => Err(vec![])
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
        fn test_solve(starting_hash in any::<[u8; 32]>()) {
            if let Some((e_l, e_h)) = solve(&starting_hash, 0) {
                if let Some((t_l, t_h)) = solve(&starting_hash, e_l) {
                    assert_eq!((e_l, e_h), (t_l, t_h), "user contract not consistent. {e_l} != {t_l} or {e_h} != {t_h}");
                    
                    if let Some((p_l, p_h)) = prover::default_solve(&starting_hash, e_l) {
                        assert_eq!(
                            (e_l, e_h), (p_l, p_h),
                            "user contract inconsistent with reference. {e_l} != {p_l} or {e_h} != {p_h}"
                        );
                    } else {
                        panic!("Reference solve failed where user solve succeeded");
                    }
                } else {
                    panic!("Second solve failed where first succeeded");
                }
            } else {
                if let Some((p_l, p_h)) = prover::default_solve(&starting_hash, 0) {
                    panic!("User solve failed but reference solve succeeded with ({p_l}, {p_h})");
                }
            }
        }
    }
}