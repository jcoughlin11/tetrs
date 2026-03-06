use ggez::{
    Context, ContextBuilder, GameResult, conf, event,
    event::EventHandler,
    graphics::{Canvas, Color, DrawMode, DrawParam, Mesh, Rect},
    input::keyboard::{KeyCode, KeyInput},
};
use rand;

// ===========================================
//                 Constants
// ===========================================
// These are usize b/c they're used as loop bound and array indices, which require int types
// in rust. They're cast to floats when needed on the fly
const COLS: usize = 10;
const ROWS: usize = 20;
const CELL_SIZE: f32 = 30.0;
const SCREEN_X: f32 = COLS as f32 * CELL_SIZE;
const SCREEN_Y: f32 = ROWS as f32 * CELL_SIZE;
const DROP_INTERVAL: f32 = 0.5; // Seconds

// ===========================================
//                 Tetrominos
// ===========================================
#[derive(Clone, Copy)]
enum TetrominoKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

fn tetromino_color(kind: TetrominoKind) -> Color {
    match kind {
        TetrominoKind::I => Color::from_rgb(0, 240, 240),
        TetrominoKind::O => Color::from_rgb(240, 240, 0),
        TetrominoKind::T => Color::from_rgb(160, 0, 240),
        TetrominoKind::S => Color::from_rgb(0, 240, 0),
        TetrominoKind::Z => Color::from_rgb(240, 0, 0),
        TetrominoKind::J => Color::from_rgb(0, 0, 240),
        TetrominoKind::L => Color::from_rgb(240, 160, 0),
    }
}

fn tetromino_cells(kind: TetrominoKind) -> [(i32, i32); 4] {
    match kind {
        TetrominoKind::I => [(0, 0), (0, 1), (0, 2), (0, 3)],
        TetrominoKind::O => [(0, 0), (0, 1), (1, 0), (1, 1)],
        TetrominoKind::T => [(0, 0), (0, 1), (0, 2), (1, 1)],
        TetrominoKind::S => [(0, 1), (0, 2), (1, 0), (1, 1)],
        TetrominoKind::Z => [(0, 0), (0, 1), (1, 1), (1, 2)],
        TetrominoKind::J => [(0, 0), (1, 0), (1, 1), (1, 2)],
        TetrominoKind::L => [(0, 2), (1, 0), (1, 1), (1, 2)],
    }
}

fn random_kind() -> TetrominoKind {
    match rand::random::<u8>() % 7 {
        0 => TetrominoKind::I,
        1 => TetrominoKind::O,
        2 => TetrominoKind::T,
        3 => TetrominoKind::S,
        4 => TetrominoKind::Z,
        5 => TetrominoKind::J,
        _ => TetrominoKind::L,
    }
}

struct Tetromino {
    cells: [(i32, i32); 4], // Offsets from tetromino origin (row, col)
    color: Color,
    row: i32, // Tetromino origin row on board
    col: i32, // Tetromino origin column on board
}

impl Tetromino {
    fn new(kind: TetrominoKind) -> Self {
        Tetromino {
            cells: tetromino_cells(kind),
            color: tetromino_color(kind),
            row: 0,
            col: COLS as i32 / 2 - 2,
        }
    }

    fn absolute_cells(&self) -> [(i32, i32); 4] {
        self.cells.map(|(r, c)| (self.row + r, self.col + c))
    }

    fn can_move_down(&self, board: &[[Option<Color>; COLS]; ROWS]) -> bool {
        self.absolute_cells()
            .iter()
            .all(|(r, c)| r + 1 < ROWS as i32 && board[(r + 1) as usize][*c as usize].is_none())
    }
    fn can_move_left(&self, board: &[[Option<Color>; COLS]; ROWS]) -> bool {
        self.absolute_cells()
            .iter()
            .all(|(r, c)| c - 1 >= 0 && board[*r as usize][(c - 1) as usize].is_none())
    }

    fn can_move_right(&self, board: &[[Option<Color>; COLS]; ROWS]) -> bool {
        self.absolute_cells()
            .iter()
            .all(|(r, c)| c + 1 < COLS as i32 && board[*r as usize][(c + 1) as usize].is_none())
    }

    fn rotate(&mut self, board: &[[Option<Color>; COLS]; ROWS]) {
        // 90 degree clockwise rotation
        let rotated: [(i32, i32); 4] = self.cells.map(|(r, c)| (c, -r));

        // Normalize so min offsets start at 0
        let min_r = rotated.iter().map(|(r, _)| *r).min().unwrap();
        let min_c = rotated.iter().map(|(_, c)| *c).min().unwrap();
        let normalized: [(i32, i32); 4] = rotated.map(|(r, c)| (r - min_r, c - min_c));

        // Try the rotation with wall kicks (so pieces up against the wall can still be rotated)
        for kick in [0i32, -1, 1, -2, 2] {
            let valid = normalized.iter().all(|(r, c)| {
                let ar = self.row + r;
                let ac = self.col + c + kick;
                ar >= 0
                    && ar < ROWS as i32
                    && ac >= 0
                    && ac < COLS as i32
                    && board[ar as usize][ac as usize].is_none()
            });

            if valid {
                self.cells = normalized;
                self.col += kick;
                return;
            }
        }
        // If no kick works, rotation is silently ignored
    }

    fn lock(&self, board: &mut [[Option<Color>; COLS]; ROWS]) {
        for (r, c) in self.absolute_cells() {
            board[r as usize][c as usize] = Some(self.color);
        }
    }

    fn overlaps(&self, board: &[[Option<Color>; COLS]; ROWS]) -> bool {
        self.absolute_cells()
            .iter()
            .any(|(r, c)| board[*r as usize][*c as usize].is_some())
    }
}

// ===========================================
//                 GameState
// ===========================================
struct GameState {
    active: Tetromino,
    drop_timer: f32,
    board: [[Option<Color>; COLS]; ROWS],
    game_over: bool,
}

impl GameState {
    fn new() -> Self {
        GameState {
            active: Tetromino::new(random_kind()),
            drop_timer: 0.0,
            board: [[None; COLS]; ROWS],
            game_over: false,
        }
    }

    fn clear_lines(&mut self) {
        let mut row = ROWS as i32 - 1;
        while row >= 0 {
            let full = self.board[row as usize].iter().all(|cell| cell.is_some());
            if full {
                // Shift every row down by 1
                for r in (1..=row as usize).rev() {
                    self.board[r] = self.board[r - 1];
                }
                self.board[0] = [None; COLS];
            } else {
                row -= 1;
            }
        }
    }
}

// -----
// EventHandler - trait
// -----
// Ggez requires that the EventHandler trait be implemented for GameState
impl EventHandler for GameState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        if self.game_over {
            return Ok(());
        }
        let dt = ctx.time.delta().as_secs_f32();
        self.drop_timer += dt;
        if self.drop_timer >= DROP_INTERVAL {
            self.drop_timer = 0.0;
            if self.active.can_move_down(&self.board) {
                self.active.row += 1;
            } else {
                self.active.lock(&mut self.board);
                self.clear_lines();
                self.active = Tetromino::new(random_kind());
                if self.active.overlaps(&self.board) {
                    self.game_over = true;
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = Canvas::from_frame(ctx, Color::BLACK);
        let grid_color = Color::from_rgb(40, 40, 40);

        // Draw each individual cell with boundaries
        for row in 0..ROWS {
            for col in 0..COLS {
                let cell_rect = Rect::new(
                    col as f32 * CELL_SIZE,
                    row as f32 * CELL_SIZE,
                    CELL_SIZE,
                    CELL_SIZE,
                );
                let mesh = Mesh::new_rectangle(ctx, DrawMode::stroke(1.0), cell_rect, grid_color)?;

                canvas.draw(&mesh, DrawParam::default());
            }
        }

        // Draw locked board cells
        for row in 0..ROWS {
            for col in 0..COLS {
                if let Some(color) = self.board[row][col] {
                    let cell_rect = Rect::new(
                        col as f32 * CELL_SIZE,
                        row as f32 * CELL_SIZE,
                        CELL_SIZE,
                        CELL_SIZE,
                    );
                    let mesh = Mesh::new_rectangle(ctx, DrawMode::fill(), cell_rect, color)?;
                    canvas.draw(&mesh, DrawParam::default());
                }
            }
        }

        // Draw active piece
        for (r, c) in self.active.absolute_cells() {
            if r >= 0 && r < ROWS as i32 && c >= 0 && c < COLS as i32 {
                let cell_rect = Rect::new(
                    c as f32 * CELL_SIZE,
                    r as f32 * CELL_SIZE,
                    CELL_SIZE,
                    CELL_SIZE,
                );
                let mesh =
                    Mesh::new_rectangle(ctx, DrawMode::fill(), cell_rect, self.active.color)?;
                canvas.draw(&mesh, DrawParam::default());
            }
        }

        if self.game_over {
            let text = ggez::graphics::Text::new("GAME OVER");
            canvas.draw(&text, DrawParam::default().dest([80.0, SCREEN_Y / 2.0]));
        }
        canvas.finish(ctx)?;
        Ok(())
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        input: KeyInput,
        _repeated: bool,
    ) -> GameResult {
        match input.keycode {
            Some(KeyCode::Left) => {
                if self.active.can_move_left(&self.board) {
                    self.active.col -= 1;
                }
            }

            Some(KeyCode::Right) => {
                if self.active.can_move_right(&self.board) {
                    self.active.col += 1;
                }
            }

            Some(KeyCode::Down) => {
                if self.active.can_move_down(&self.board) {
                    self.active.row += 1;
                    self.drop_timer = 0.0;
                }
            }

            Some(KeyCode::Up) => {
                self.active.rotate(&self.board);
            }
            _ => {}
        }

        Ok(())
    }
}

// ===========================================
//                    Main
// ===========================================
fn main() -> GameResult {
    let (ctx, event_loop) = ContextBuilder::new("tetrs", "author")
        .window_setup(conf::WindowSetup::default().title("Tetrs"))
        .window_mode(conf::WindowMode::default().dimensions(SCREEN_X, SCREEN_Y))
        .build()?;

    let state = GameState::new();
    event::run(ctx, event_loop, state)
}
