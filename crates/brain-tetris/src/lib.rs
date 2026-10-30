#![no_std]
extern crate alloc;

mod paint;
mod rotation;
use reflex::{Action, Application, Event, peg::key};

const WIDTH: usize = 10;
const VISIBLE_HEIGHT: usize = 20;
const HIDDEN: usize = 20;
const HEIGHT: usize = HIDDEN + VISIBLE_HEIGHT;
const LOCK_MS: u64 = 500;
const LOCK_RESETS: u8 = 15;
const DAS_MS: u64 = 150;
const ARR_MS: u64 = 40;
const GRAVITY_MS: [u64; 15] = [
    1000, 793, 618, 473, 355, 262, 190, 135, 94, 64, 43, 28, 18, 11, 7,
];
const SHAPES: [[(i32, i32); 4]; 7] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
];

#[derive(Clone, Copy)]
struct Piece {
    kind: usize,
    rotation: u8,
    x: i32,
    y: i32,
}
impl Piece {
    fn new(kind: usize) -> Self {
        Self {
            kind,
            rotation: 0,
            x: 3,
            y: HIDDEN as i32 - 2,
        }
    }
    fn cells(self) -> [(i32, i32); 4] {
        SHAPES[self.kind].map(|(mut x, mut y)| {
            if self.kind != 1 {
                let edge = if self.kind == 0 { 3 } else { 2 };
                for _ in 0..self.rotation {
                    (x, y) = (edge - y, x);
                }
            }
            (x + self.x, y + self.y)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spin {
    None,
    Mini,
    Full,
}

pub struct Tetris {
    board: [[u8; WIDTH]; HEIGHT],
    piece: Piece,
    next: [usize; 5],
    hold: Option<usize>,
    hold_used: bool,
    bag: [usize; 7],
    remaining: usize,
    random: u32,
    score: u64,
    lines: u32,
    combo: u32,
    back_to_back: bool,
    last_clear: &'static str,
    last_points: u64,
    last_b2b: bool,
    perfect_clear: bool,
    last_rotation: Option<usize>,
    gravity_ms: u64,
    lock_ms: u64,
    lock_resets: u8,
    lock_started: bool,
    lowest_y: i32,
    horizontal: i32,
    repeat_ms: u64,
    paused: bool,
    over: bool,
    keys: [bool; 13],
}
impl Tetris {
    pub fn new(seed: u32) -> Self {
        let mut game = Self {
            board: [[0; WIDTH]; HEIGHT],
            piece: Piece::new(0),
            next: [0; 5],
            hold: None,
            hold_used: false,
            bag: [0, 1, 2, 3, 4, 5, 6],
            remaining: 0,
            random: seed,
            score: 0,
            lines: 0,
            combo: 0,
            back_to_back: false,
            last_clear: "",
            last_points: 0,
            last_b2b: false,
            perfect_clear: false,
            last_rotation: None,
            gravity_ms: 0,
            lock_ms: 0,
            lock_resets: 0,
            lock_started: false,
            lowest_y: 0,
            horizontal: 0,
            repeat_ms: 0,
            paused: false,
            over: false,
            keys: [false; 13],
        };
        let first = game.take_piece();
        game.spawn(first);
        for i in 0..game.next.len() {
            game.next[i] = game.take_piece();
        }
        game
    }
    fn take_piece(&mut self) -> usize {
        if self.remaining == 0 {
            for i in (1..7).rev() {
                self.random = self.random.wrapping_mul(1664525).wrapping_add(1013904223);
                self.bag.swap(i, ((self.random >> 16) as usize) % (i + 1));
            }
            self.remaining = 7;
        }
        self.remaining -= 1;
        self.bag[self.remaining]
    }
    fn spawn(&mut self, kind: usize) {
        self.piece = Piece::new(kind);
        self.last_rotation = None;
        self.gravity_ms = 0;
        self.lock_ms = 0;
        self.lock_resets = 0;
        self.lock_started = false;
        self.over = !self.fits(self.piece);
        let entry = Piece {
            y: self.piece.y + 1,
            ..self.piece
        };
        if !self.over && self.fits(entry) {
            self.piece = entry;
        }
        self.lowest_y = self.piece.y;
    }
    fn spawn_next(&mut self) {
        let kind = self.next[0];
        self.next.rotate_left(1);
        self.next[4] = self.take_piece();
        self.spawn(kind);
    }
    fn hold(&mut self) {
        if self.hold_used {
            return;
        }
        match self.hold.replace(self.piece.kind) {
            Some(kind) => self.spawn(kind),
            None => self.spawn_next(),
        }
        self.hold_used = true;
    }
    fn occupied(&self, x: i32, y: i32) -> bool {
        !(0..WIDTH as i32).contains(&x)
            || !(0..HEIGHT as i32).contains(&y)
            || self.board[y as usize][x as usize] != 0
    }
    fn fits(&self, piece: Piece) -> bool {
        piece.cells().iter().all(|&(x, y)| !self.occupied(x, y))
    }
    fn grounded(&self) -> bool {
        !self.fits(Piece {
            y: self.piece.y + 1,
            ..self.piece
        })
    }
    fn moved(&mut self, piece: Piece, rotation: Option<usize>, manipulation: bool) {
        self.lock_started |= self.grounded();
        if manipulation && self.lock_started && self.lock_resets < LOCK_RESETS {
            self.lock_ms = 0;
            self.lock_resets += 1;
        }
        self.piece = piece;
        self.last_rotation = rotation;
        // Only reaching a new lowest row renews the move-reset budget.
        if piece.y > self.lowest_y {
            self.lowest_y = piece.y;
            self.lock_ms = 0;
            self.lock_resets = 0;
            self.lock_started = false;
        }
    }
    fn shift(&mut self, dx: i32, dy: i32) -> bool {
        let candidate = Piece {
            x: self.piece.x + dx,
            y: self.piece.y + dy,
            ..self.piece
        };
        if !self.fits(candidate) {
            return false;
        }
        self.moved(candidate, None, dx != 0);
        true
    }
    fn rotate(&mut self, clockwise: bool) {
        let to = (self.piece.rotation + if clockwise { 1 } else { 3 }) % 4;
        for (kick, (dx, dy)) in rotation::kicks(self.piece.kind, self.piece.rotation, to)
            .into_iter()
            .enumerate()
        {
            let candidate = Piece {
                x: self.piece.x + dx,
                y: self.piece.y - dy,
                rotation: to,
                ..self.piece
            };
            if self.fits(candidate) {
                self.moved(candidate, Some(kick), true);
                break;
            }
        }
    }
    fn spin(&self) -> Spin {
        if self.piece.kind != 2 || self.last_rotation.is_none() {
            return Spin::None;
        }
        let x = self.piece.x;
        let y = self.piece.y;
        let corners =
            [(x, y), (x + 2, y), (x + 2, y + 2), (x, y + 2)].map(|(x, y)| self.occupied(x, y));
        if corners.iter().filter(|&&occupied| occupied).count() < 3 {
            return Spin::None;
        }
        let front = self.piece.rotation as usize;
        if (corners[front] && corners[(front + 1) % 4]) || self.last_rotation == Some(4) {
            Spin::Full
        } else {
            Spin::Mini
        }
    }
    fn lock(&mut self) {
        let spin = self.spin();
        let cells = self.piece.cells();
        for (x, y) in cells {
            self.board[y as usize][x as usize] = self.piece.kind as u8 + 1;
        }
        if cells.iter().all(|&(_, y)| y < HIDDEN as i32) {
            self.over = true;
            return;
        }
        let mut cleared = 0;
        let mut destination = HEIGHT;
        for source in (0..HEIGHT).rev() {
            if self.board[source].contains(&0) {
                destination -= 1;
                self.board[destination] = self.board[source];
            } else {
                cleared += 1;
            }
        }
        self.board[..destination].fill([0; WIDTH]);
        self.award(spin, cleared);
        self.hold_used = false;
        self.spawn_next();
    }
    fn award(&mut self, spin: Spin, cleared: usize) {
        let base = match spin {
            Spin::None => [0, 100, 300, 500, 800][cleared],
            Spin::Mini => [100, 200, 400][cleared],
            Spin::Full => [400, 800, 1200, 1600][cleared],
        };
        let difficult = cleared > 0 && (cleared == 4 || spin != Spin::None);
        self.last_b2b = difficult && self.back_to_back;
        if cleared > 0 {
            self.back_to_back = difficult;
            self.combo = self.combo.saturating_add(1);
        } else {
            self.combo = 0;
        }
        self.perfect_clear = cleared > 0 && self.board.iter().flatten().all(|&cell| cell == 0);
        let perfect = if self.perfect_clear {
            if cleared == 4 && self.last_b2b {
                3200
            } else {
                [0, 800, 1200, 1600, 2000][cleared]
            }
        } else {
            0
        };
        let bonus = u64::from(self.combo.saturating_sub(1)) * 50;
        self.last_points = (base + if self.last_b2b { base / 2 } else { 0 } + bonus + perfect)
            * u64::from(self.level());
        self.score = self.score.saturating_add(self.last_points);
        self.lines = self.lines.saturating_add(cleared as u32);
        self.last_clear = match spin {
            Spin::None => ["", "SINGLE", "DOUBLE", "TRIPLE", "TETRIS"][cleared],
            Spin::Mini => ["T-SPIN MINI", "MINI SINGLE", "MINI DOUBLE"][cleared],
            Spin::Full => ["T-SPIN", "T-SPIN SINGLE", "T-SPIN DOUBLE", "T-SPIN TRIPLE"][cleared],
        };
    }
    fn level(&self) -> u32 {
        self.lines / 10 + 1
    }
    fn soft_drop(&mut self) {
        if self.shift(0, 1) {
            self.score = self.score.saturating_add(1);
        }
    }
    pub fn advance(&mut self, milliseconds: u64) -> Action {
        if self.paused || self.over {
            return Action::None;
        }
        let mut dirty = false;
        // Millisecond steps keep gravity, DAS/ARR and locking independent of timer delivery batches.
        for _ in 0..milliseconds {
            if self.horizontal != 0 {
                self.repeat_ms += 1;
                if self.repeat_ms >= DAS_MS && (self.repeat_ms - DAS_MS).is_multiple_of(ARR_MS) {
                    dirty |= self.shift(self.horizontal, 0);
                }
            }
            let grounded = self.grounded();
            self.lock_started |= grounded;
            if grounded {
                self.lock_ms += 1;
            }
            self.gravity_ms += 1;
            let interval = GRAVITY_MS[(self.level() as usize - 1).min(GRAVITY_MS.len() - 1)];
            let interval = if self.keys[2] {
                interval.min(20)
            } else {
                interval
            };
            if self.gravity_ms >= interval {
                self.gravity_ms = 0;
                if self.shift(0, 1) {
                    if self.keys[2] {
                        self.score = self.score.saturating_add(1);
                    }
                    dirty = true;
                }
            }
            if self.grounded() && (self.lock_ms >= LOCK_MS || self.lock_resets >= LOCK_RESETS) {
                self.lock();
                dirty = true;
                if self.over {
                    break;
                }
            }
        }
        if dirty { Action::Redraw } else { Action::None }
    }
}
impl Application for Tetris {
    fn event(&mut self, event: Event) -> Action {
        let Event::Key { code, down, .. } = event else {
            return Action::None;
        };
        if code == key::ESCAPE && down {
            return Action::Exit;
        }
        let code = if (b'a' as u16..=b'z' as u16).contains(&code) {
            code - 32
        } else {
            code
        };
        let Some(index) = [
            key::LEFT,
            key::RIGHT,
            key::DOWN,
            key::UP,
            b'X' as u16,
            b'Z' as u16,
            key::SPACE,
            b'C' as u16,
            key::SHIFT,
            key::ENTER,
            b'P' as u16,
            key::BACKSPACE,
            0x7f,
        ]
        .iter()
        .position(|&binding| binding == code) else {
            return Action::None;
        };
        let held = self.keys[index];
        self.keys[index] = down;
        if !down {
            if index < 2 && self.horizontal == if index == 0 { -1 } else { 1 } {
                self.horizontal = if self.keys[1 - index] {
                    -self.horizontal
                } else {
                    0
                };
                self.repeat_ms = 0;
            }
            return Action::None;
        }
        if held {
            return Action::None;
        }
        if index >= 9 {
            if index >= 11 || (self.over && index == 9) {
                let keys = self.keys;
                *self = Self::new(self.random);
                self.keys = keys;
                self.keys[..3].fill(false);
            } else if !self.over {
                self.paused = !self.paused;
                self.horizontal = 0;
                self.repeat_ms = 0;
                self.keys[..3].fill(false);
            }
            return Action::Redraw;
        }
        if self.over || self.paused {
            return Action::None;
        }
        match index {
            0 | 1 => {
                self.horizontal = if index == 0 { -1 } else { 1 };
                self.repeat_ms = 0;
                self.shift(self.horizontal, 0);
            }
            2 => {
                self.soft_drop();
                self.gravity_ms = 0;
            }
            4 => self.rotate(true),
            5 => self.rotate(false),
            3 | 6 => {
                while self.shift(0, 1) {
                    self.score = self.score.saturating_add(2);
                }
                self.lock();
            }
            7 | 8 => self.hold(),
            _ => unreachable!(),
        }
        if !self.over && self.lock_resets >= LOCK_RESETS && self.grounded() {
            self.lock();
        }
        Action::Redraw
    }
    fn draw(&self, pixels: &mut [u16]) {
        paint::draw(self, pixels);
    }
}
