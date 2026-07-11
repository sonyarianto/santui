use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

fn piece_char(c: char) -> char {
    match c {
        'K' => '♔',
        'Q' => '♕',
        'R' => '♖',
        'B' => '♗',
        'N' => '♘',
        'P' => '♙',
        'k' => '♚',
        'q' => '♛',
        'r' => '♜',
        'b' => '♝',
        'n' => '♞',
        'p' => '♟',
        _ => '·',
    }
}

fn is_white(c: char) -> bool {
    matches!(c, 'K' | 'Q' | 'R' | 'B' | 'N' | 'P')
}

fn is_black(c: char) -> bool {
    matches!(c, 'k' | 'q' | 'r' | 'b' | 'n' | 'p')
}

fn alg_to_pos(alg: &str) -> Option<(usize, usize)> {
    let b = alg.as_bytes();
    if b.len() != 2 {
        return None;
    }
    let col = (b[0].wrapping_sub(b'a')) as usize;
    let row = (8usize).wrapping_sub((b[1].wrapping_sub(b'0')) as usize);
    if col > 7 || row > 7 {
        return None;
    }
    Some((row, col))
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    board: [[char; 8]; 8],
    turn_white: bool,
    input: String,
    status: String,
    history: Vec<(String, String)>,
}

impl Default for App {
    fn default() -> Self {
        let mut b = [['.'; 8]; 8];
        b[0] = ['r', 'n', 'b', 'q', 'k', 'b', 'n', 'r'];
        b[1] = ['p'; 8];
        b[6] = ['P'; 8];
        b[7] = ['R', 'N', 'B', 'Q', 'K', 'B', 'N', 'R'];
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            board: b,
            turn_white: true,
            input: String::new(),
            status: "White to move".to_string(),
            history: Vec::new(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => {
                if !self.input.is_empty() {
                    self.input.clear();
                    true
                } else {
                    false
                }
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Enter => {
                self.make_move();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                if self.input.len() < 5 {
                    self.input.push(c);
                }
                true
            }
            IpcKey::Char('u') => {
                self.undo_move();
                true
            }
            _ => true,
        }
    }

    fn make_move(&mut self) {
        let input = self.input.trim().to_lowercase();
        self.input.clear();
        if input.len() != 4 {
            self.status = String::from("Invalid input. Use format like e2e4");
            return;
        }
        let from = match alg_to_pos(&input[0..2]) {
            Some(p) => p,
            None => {
                self.status = String::from("Invalid from square");
                return;
            }
        };
        let to = match alg_to_pos(&input[2..4]) {
            Some(p) => p,
            None => {
                self.status = String::from("Invalid to square");
                return;
            }
        };
        let piece = self.board[from.0][from.1];
        if piece == '.' {
            self.status = String::from("No piece on source square");
            return;
        }
        if self.turn_white && !is_white(piece) {
            self.status = String::from("It's White's turn");
            return;
        }
        if !self.turn_white && !is_black(piece) {
            self.status = String::from("It's Black's turn");
            return;
        }
        let target = self.board[to.0][to.1];
        if self.turn_white && is_white(target) {
            self.status = String::from("Cannot capture own piece");
            return;
        }
        if !self.turn_white && is_black(target) {
            self.status = String::from("Cannot capture own piece");
            return;
        }
        if !self.validate_move(from, to, piece) {
            return;
        }
        self.history
            .push((input.clone(), format!("{} {:?}->{:?}", piece, from, to)));
        self.board[to.0][to.1] = piece;
        self.board[from.0][from.1] = '.';
        self.turn_white = !self.turn_white;
        let side = if self.turn_white { "White" } else { "Black" };
        self.status = format!("{} to move", side);
    }

    fn validate_move(&mut self, from: (usize, usize), to: (usize, usize), piece: char) -> bool {
        let (fr, fc) = from;
        let (tr, tc) = to;
        let dr = tr as isize - fr as isize;
        let dc = tc as isize - fc as isize;
        match piece.to_ascii_uppercase() {
            'P' => {
                let forward = if is_white(piece) { -1 } else { 1 };
                let start_row = if is_white(piece) { 6 } else { 1 };
                if dc == 0 && dr == forward && self.board[tr][tc] == '.' {
                    return true;
                }
                if dc == 0
                    && dr == 2 * forward
                    && fr == start_row
                    && self.board[tr][tc] == '.'
                    && self.board[(fr as isize + forward) as usize][fc] == '.'
                {
                    return true;
                }
                if (dc == 1 || dc == -1) && dr == forward && self.board[tr][tc] != '.' {
                    return true;
                }
                self.status = String::from("Invalid pawn move");
                false
            }
            'R' => {
                if dr != 0 && dc != 0 {
                    self.status = String::from("Rook moves horizontally or vertically");
                    return false;
                }
                self.path_clear(from, to)
            }
            'B' => {
                if dr.abs() != dc.abs() || dr == 0 {
                    self.status = String::from("Bishop moves diagonally");
                    return false;
                }
                self.path_clear(from, to)
            }
            'Q' => {
                if dr == 0 && dc == 0 {
                    self.status = String::from("Queen must move");
                    return false;
                }
                if dr != 0 && dc != 0 && dr.abs() != dc.abs() {
                    self.status =
                        String::from("Queen moves horizontally, vertically, or diagonally");
                    return false;
                }
                self.path_clear(from, to)
            }
            'N' => {
                let valid = (dr.abs(), dc.abs()) == (1, 2) || (dr.abs(), dc.abs()) == (2, 1);
                if !valid {
                    self.status = String::from("Invalid knight move");
                }
                valid
            }
            'K' => {
                if dr.abs() > 1 || dc.abs() > 1 {
                    self.status = String::from("King moves one square");
                    return false;
                }
                true
            }
            _ => {
                self.status = format!("Unknown piece: {}", piece);
                false
            }
        }
    }

    fn path_clear(&mut self, from: (usize, usize), to: (usize, usize)) -> bool {
        let (fr, fc) = (from.0 as isize, from.1 as isize);
        let (tr, tc) = (to.0 as isize, to.1 as isize);
        let dr = (tr - fr).signum();
        let dc = (tc - fc).signum();
        let mut r = fr + dr;
        let mut c = fc + dc;
        while (r, c) != (tr, tc) {
            if self.board[r as usize][c as usize] != '.' {
                self.status = String::from("Path is blocked");
                return false;
            }
            r += dr;
            c += dc;
        }
        true
    }

    fn undo_move(&mut self) {
        if let Some((notation, _)) = self.history.pop() {
            let from = alg_to_pos(&notation[0..2]);
            let to = alg_to_pos(&notation[2..4]);
            if let (Some(f), Some(t)) = (from, to) {
                let piece = self.board[t.0][t.1];
                self.board[f.0][f.1] = piece;
                self.board[t.0][t.1] = '.';
                self.turn_white = !self.turn_white;
                self.status = format!("Undid {}", notation);
            }
        } else {
            self.status = String::from("No moves to undo");
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(16);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": String::from(" Chess "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": format!("Turn: {}", if self.turn_white { "White" } else { "Black" }),
            "fg": t.text, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let board_start_x = 4u16;
        let board_start_y = 3u16;
        let files = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];

        for (ri, row) in self.board.iter().enumerate() {
            let rank = 8 - ri;
            cmds.push(json!({"Text": {
                "x": board_start_x - 2, "y": board_start_y + ri as u16,
                "text": format!("{}", rank),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
            for (ci, &piece) in row.iter().enumerate() {
                let is_light = (ri + ci) % 2 == 0;
                let bg = if is_light {
                    t.background_panel
                } else {
                    [40, 40, 40]
                };
                let fg = if is_white(piece) {
                    t.text
                } else if is_black(piece) {
                    t.error
                } else {
                    t.text_muted
                };
                cmds.push(json!({"Text": {
                    "x": board_start_x + ci as u16 * 2, "y": board_start_y + ri as u16,
                    "text": format!("{}", piece_char(piece)),
                    "fg": fg, "bg": bg, "bold": false, "modifiers": 0,
                }}));
            }
        }

        for (ci, &f) in files.iter().enumerate() {
            cmds.push(json!({"Text": {
                "x": board_start_x + ci as u16 * 2, "y": board_start_y + 8,
                "text": format!("{}", f),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        }

        let info_x = board_start_x + 18u16;
        cmds.push(json!({"Text": {
            "x": info_x, "y": 3, "text": format!("> {}", self.input),
            "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": info_x, "y": 5, "text": String::from("Type move as"),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": info_x, "y": 6, "text": String::from("e.g. e2e4"),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        if !self.history.is_empty() {
            let start = self.history.len().saturating_sub(5);
            let recent: Vec<&str> = self
                .history
                .iter()
                .skip(start)
                .map(|(n, _)| n.as_str())
                .collect();
            cmds.push(json!({"Text": {
                "x": info_x, "y": 8, "text": String::from("Recent moves:"),
                "fg": t.text_muted, "bg": null, "bold": true, "modifiers": 0,
            }}));
            for (i, n) in recent.iter().enumerate() {
                cmds.push(json!({"Text": {
                    "x": info_x, "y": 9 + i as u16, "text": format!("  {}", n),
                    "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                }}));
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("Type algebraic notation (e.g. e2e4) + Enter · u: undo · Esc: clear/close"),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }
}

fn default_theme() -> ThemeData {
    ThemeData {
        text: [220; 3],
        text_muted: [140; 3],
        accent: [180; 3],
        highlight: [220; 3],
        logo: [255; 3],
        background: [0; 3],
        background_panel: [20; 3],
        background_overlay: [10; 3],
        border: [150; 3],
        success: [127, 216, 143],
        error: [224, 108, 117],
        inverted_text: [20; 3],
    }
}

fn palette_commands() -> Value {
    json!([["Plugins", "Chess"]])
}

fn key_hints() -> Value {
    json!([["u", "undo move"], ["Enter", "make move"],])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = json!({
        "commands": commands_val, "hints": key_hints(), "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
    });
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg = serde_json::from_str::<HostMsg>(trimmed);
        let consumed = match msg {
            Ok(HostMsg::Init { theme, area, .. }) => {
                app.theme = theme;
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Resize { area }) => {
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::ThemeChange { theme }) => {
                app.theme = theme;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Key { key, modifiers }) => app.handle_key(key, modifiers),
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[chess] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
