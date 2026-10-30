use crate::{HIDDEN, Piece, SHAPES, Tetris};
use alloc::format;
use reflex::{
    display::{HEIGHT, WIDTH},
    text,
};

const BG: u16 = 0xf7bd;
const INK: u16 = 0x18e3;
const WELL: u16 = 0x1082;
const COLORS: [u16; 8] = [WELL, 0x05df, 0xfec0, 0xb29f, 0x3e67, 0xf249, 0x437f, 0xfba3];
const CELL: usize = 22;
const X: usize = 274;
const Y: usize = 20;

struct Canvas<'a>(&'a mut [u16]);
impl Canvas<'_> {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u16) {
        for row in y..(y + h).min(HEIGHT) {
            self.0[row * WIDTH + x..row * WIDTH + (x + w).min(WIDTH)].fill(color);
        }
    }
    fn text(&mut self, x: usize, y: usize, value: &str, scale: usize) {
        text::draw(
            |x, y, color| self.0[y * WIDTH + x] = color,
            x,
            y,
            value,
            INK,
            BG,
            scale,
        );
    }
    fn block(&mut self, x: usize, y: usize, size: usize, color: u16) {
        self.rect(x + 1, y + 1, size - 2, size - 2, color);
        self.rect(x + 3, y + 3, size - 6, 2, 0xffff);
    }
    fn preview(&mut self, x: usize, y: usize, size: usize, kind: usize, used: bool) {
        for (dx, dy) in SHAPES[kind] {
            self.block(
                x + dx as usize * size,
                y + dy as usize * size,
                size,
                if used { 0x7bef } else { COLORS[kind + 1] },
            );
        }
    }
}
pub fn draw(game: &Tetris, pixels: &mut [u16]) {
    assert_eq!(pixels.len(), WIDTH * HEIGHT);
    pixels.fill(BG);
    let mut c = Canvas(pixels);
    c.text(28, 24, "TETRIS", 2);
    c.text(28, 94, if game.hold_used { "HOLD USED" } else { "HOLD" }, 1);
    if let Some(kind) = game.hold {
        c.preview(28, 128, CELL, kind, game.hold_used);
    }
    c.text(28, 194, "SCORE", 1);
    c.text(28, 226, &format!("{}", game.score), 1);
    c.text(28, 270, &format!("LINES {}", game.lines), 1);
    c.text(28, 302, &format!("LEVEL {}", game.level()), 1);
    if !game.last_clear.is_empty() {
        c.text(28, 360, game.last_clear, 1);
        c.text(28, 390, &format!("+{}", game.last_points), 1);
        if game.perfect_clear {
            c.text(28, 428, "ALL CLEAR", 1);
        }
    }
    c.rect(X - 2, Y - 2, CELL * 10 + 4, CELL * 20 + 4, INK);
    c.rect(X, Y, CELL * 10, CELL * 20, WELL);
    for (y, row) in game.board[HIDDEN..].iter().enumerate() {
        for (x, &kind) in row.iter().enumerate() {
            if kind != 0 {
                c.block(X + x * CELL, Y + y * CELL, CELL, COLORS[kind as usize]);
            }
        }
    }
    if !game.over {
        let mut ghost = game.piece;
        while game.fits(Piece {
            y: ghost.y + 1,
            ..ghost
        }) {
            ghost.y += 1;
        }
        for (x, y) in ghost.cells() {
            if y < HIDDEN as i32 {
                continue;
            }
            let (x, y) = (X + x as usize * CELL, Y + (y as usize - HIDDEN) * CELL);
            c.rect(x + 1, y + 1, CELL - 2, CELL - 2, 0xad55);
            c.rect(x + 3, y + 3, CELL - 6, CELL - 6, WELL);
        }
        for (x, y) in game.piece.cells() {
            if y >= HIDDEN as i32 {
                c.block(
                    X + x as usize * CELL,
                    Y + (y as usize - HIDDEN) * CELL,
                    CELL,
                    COLORS[game.piece.kind + 1],
                );
            }
        }
    }
    c.text(530, 24, "NEXT", 1);
    for (i, &kind) in game.next.iter().enumerate() {
        c.preview(530 + i * 60, 66, 14, kind, false);
    }
    c.text(530, 134, "Left/Right  Move\nX           Rotate CW\nZ           Rotate CCW\nDown        Soft drop\nUp / Space  Hard drop\nC / Shift   Hold\nEnter / P   Pause\nDel         Reset\nEsc         Exit", 1);
    if game.back_to_back {
        c.text(
            530,
            374,
            if game.last_b2b {
                "BACK-TO-BACK +50%"
            } else {
                "BACK-TO-BACK"
            },
            1,
        );
    }
    if game.combo > 1 {
        c.text(530, 410, &format!("COMBO {}", game.combo - 1), 1);
    }
    if game.over || game.paused {
        c.rect(X + 4, 194, CELL * 10 - 8, 92, BG);
        c.text(
            X + 14,
            204,
            if game.over { "GAME OVER" } else { "PAUSED" },
            1,
        );
        c.text(
            X + 14,
            246,
            if game.over {
                "Enter  Retry"
            } else {
                "Enter  Resume"
            },
            1,
        );
    }
}
