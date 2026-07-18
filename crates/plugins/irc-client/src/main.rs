use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Clone)]
struct IrcMessage {
    sender: String,
    text: String,
}

struct IrcChannel {
    name: String,
    topic: String,
    messages: Vec<IrcMessage>,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    channels: Vec<IrcChannel>,
    selected_channel: usize,
    message_scroll: usize,
    input: String,
    nick: String,
    status: String,
    server: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            selected_channel: 0,
            message_scroll: 0,
            input: String::new(),
            nick: String::from("guest"),
            server: String::from("mock.irc.local"),
            status: String::from("Connected (mock mode). Type to chat, /join to join channels."),
            channels: vec![
                IrcChannel {
                    name: String::from("#general"),
                    topic: String::from("General discussion"),
                    messages: vec![
                        IrcMessage {
                            sender: String::from("alice"),
                            text: String::from("Hey everyone! Welcome to #general"),
                        },
                        IrcMessage {
                            sender: String::from("bob"),
                            text: String::from("hi alice, how's it going?"),
                        },
                        IrcMessage {
                            sender: String::from("carol"),
                            text: String::from("Has anyone tried the new Santui plugins?"),
                        },
                        IrcMessage {
                            sender: String::from("alice"),
                            text: String::from("Yes! The MySQL Browser is pretty cool"),
                        },
                        IrcMessage {
                            sender: String::from("bob"),
                            text: String::from("I've been using the IRC client ;)"),
                        },
                        IrcMessage {
                            sender: String::from("carol"),
                            text: String::from("lol nice. What about the weather plugin?"),
                        },
                        IrcMessage {
                            sender: String::from("alice"),
                            text: String::from("It's great for planning outdoor activities"),
                        },
                    ],
                },
                IrcChannel {
                    name: String::from("#random"),
                    topic: String::from("Random off-topic chatter"),
                    messages: vec![
                        IrcMessage {
                            sender: String::from("dave"),
                            text: String::from("Anyone watching anything good?"),
                        },
                        IrcMessage {
                            sender: String::from("eve"),
                            text: String::from("Just finished Dune Part 2 - amazing!"),
                        },
                        IrcMessage {
                            sender: String::from("dave"),
                            text: String::from("Nice! I need to catch up on that"),
                        },
                    ],
                },
                IrcChannel {
                    name: String::from("#dev"),
                    topic: String::from("Development and tech talk"),
                    messages: vec![
                        IrcMessage {
                            sender: String::from("frank"),
                            text: String::from("Working on a new Rust project today"),
                        },
                        IrcMessage {
                            sender: String::from("grace"),
                            text: String::from("What framework are you using?"),
                        },
                        IrcMessage {
                            sender: String::from("frank"),
                            text: String::from("Just ratatui for the UI, it's a TUI app"),
                        },
                        IrcMessage {
                            sender: String::from("grace"),
                            text: String::from("Nice, ratatui is great for terminal apps"),
                        },
                    ],
                },
            ],
        }
    }
}

impl App {
    fn process_input(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }

        if text.starts_with('/') {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            match parts[0] {
                "/join" => {
                    let channel = parts.get(1).unwrap_or(&"#new").trim().to_string();
                    if !self.channels.iter().any(|c| c.name == channel) {
                        self.channels.push(IrcChannel {
                            name: channel.clone(),
                            topic: String::from("New channel"),
                            messages: Vec::new(),
                        });
                        self.selected_channel = self.channels.len().saturating_sub(1);
                        self.status = format!("Joined {channel}");
                    } else {
                        self.selected_channel = self
                            .channels
                            .iter()
                            .position(|c| c.name == channel)
                            .unwrap_or(0);
                        self.status = format!("Switched to {channel}");
                    }
                }
                "/nick" => {
                    if let Some(new_nick) = parts.get(1) {
                        self.nick = new_nick.trim().to_string();
                        self.status = format!("Nick changed to {}", self.nick);
                    }
                }
                "/msg" => {
                    if let Some(target_and_msg) = parts.get(1) {
                        let msg_parts: Vec<&str> = target_and_msg.splitn(2, ' ').collect();
                        if msg_parts.len() == 2 {
                            let target = msg_parts[0].to_string();
                            let msg = msg_parts[1].to_string();
                            if let Some(ch) = self.channels.iter_mut().find(|c| c.name == target) {
                                ch.messages.push(IrcMessage {
                                    sender: self.nick.clone(),
                                    text: format!("[{target}] {msg}"),
                                });
                                self.status = format!("Message sent to {target}");
                            }
                        }
                    }
                }
                "/list" => {
                    let names: Vec<String> = self.channels.iter().map(|c| c.name.clone()).collect();
                    self.status = format!("Channels: {}", names.join(", "));
                }
                "/quit" | "/exit" => {
                    self.status = String::from("Disconnected");
                }
                _ => {
                    self.status = format!("Unknown command: {}", parts[0]);
                }
            }
        } else {
            if let Some(ch) = self.channels.get_mut(self.selected_channel) {
                ch.messages.push(IrcMessage {
                    sender: self.nick.clone(),
                    text: text.clone(),
                });
                self.message_scroll = ch.messages.len().saturating_sub(1);
                self.status = format!("<{}> {}", self.nick, text);
            }
        }

        self.input.clear();
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Up if modifiers.ctrl => {
                self.selected_channel = self.selected_channel.saturating_sub(1);
                self.message_scroll = 0;
                let ch = &self.channels[self.selected_channel];
                self.status = format!("#{} — {}", ch.name, ch.topic);
                true
            }
            IpcKey::Down if modifiers.ctrl => {
                let max = self.channels.len().saturating_sub(1);
                self.selected_channel = self.selected_channel.saturating_add(1).min(max);
                self.message_scroll = 0;
                let ch = &self.channels[self.selected_channel];
                self.status = format!("#{} — {}", ch.name, ch.topic);
                true
            }
            IpcKey::PageUp => {
                let available = self.area.h.saturating_sub(5) as usize;
                self.message_scroll = self.message_scroll.saturating_sub(available);
                true
            }
            IpcKey::PageDown => {
                let available = self.area.h.saturating_sub(5) as usize;
                if let Some(ch) = self.channels.get(self.selected_channel) {
                    let max = ch.messages.len().saturating_sub(1);
                    self.message_scroll = self.message_scroll.saturating_add(available).min(max);
                }
                true
            }
            IpcKey::Enter if !modifiers.ctrl => {
                self.process_input();
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.input.push(c);
                true
            }
            _ => true,
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(50);
        let h = self.area.h.max(15);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": format!(" IRC — {}@{} ", self.nick, self.server),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let sidebar_w: u16 = 20;
        let sidebar_x: u16 = 1;
        let sidebar_y: u16 = 1;
        cmds.push(json!({"Rect": {
            "x": sidebar_x, "y": sidebar_y, "w": sidebar_w, "h": h.saturating_sub(4),
            "bg": t.background_overlay,
        }}));
        cmds.push(json!({"Text": {
            "x": sidebar_x + 1, "y": sidebar_y,
            "text": String::from("Channels"),
            "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
        }}));

        for (i, ch) in self.channels.iter().enumerate() {
            let y = sidebar_y + 1 + i as u16;
            if y >= h.saturating_sub(4) - 1 {
                break;
            }
            let highlight = i == self.selected_channel;
            cmds.push(json!({"Text": {
                "x": sidebar_x + 1, "y": y,
                "text": format!(" {} {}", if highlight { "\u{25b6}" } else { " " }, ch.name),
                "fg": if highlight { t.inverted_text } else { t.text_muted },
                "bg": if highlight { Some(t.highlight) } else { Option::<[u8;3]>::None },
                "bold": highlight, "modifiers": 0,
            }}));
        }

        let msg_x = sidebar_x + sidebar_w + 1;
        let msg_w = w.saturating_sub(msg_x + 1);
        let msg_y = 1;

        if let Some(ch) = self.channels.get(self.selected_channel) {
            let line_count = h.saturating_sub(5) as usize;
            let start = self
                .message_scroll
                .saturating_sub(line_count.saturating_sub(1));
            let visible: Vec<&IrcMessage> = if ch.messages.len() > line_count {
                ch.messages.iter().skip(start).take(line_count).collect()
            } else {
                ch.messages.iter().collect()
            };

            for (i, msg) in visible.iter().enumerate() {
                let y = msg_y + i as u16;
                if y >= h.saturating_sub(4) {
                    break;
                }
                let line = format!("<{}> {}", msg.sender, msg.text);
                let fg = if msg.sender == self.nick {
                    t.success
                } else {
                    t.text
                };
                let max_w = msg_w;
                let display = if line.len() > max_w as usize {
                    format!("{}…", &line[..max_w.saturating_sub(1) as usize])
                } else {
                    line
                };
                cmds.push(json!({"Text": {
                    "x": msg_x, "y": y,
                    "text": display,
                    "fg": fg, "bg": null, "bold": false, "modifiers": 0,
                }}));
            }
        }

        let input_y = h.saturating_sub(3);
        cmds.push(json!({"Rect": {
            "x": 1, "y": input_y, "w": w.saturating_sub(2), "h": 1,
            "bg": t.background_overlay,
        }}));
        let prompt = format!(" {}> ", self.nick);
        cmds.push(json!({"Text": {
            "x": 2, "y": input_y,
            "text": prompt,
            "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
        }}));
        let input_display = if self.input.is_empty() {
            String::from("type message or /command")
        } else {
            self.input.clone()
        };
        let max_input = w.saturating_sub(4 + prompt.len() as u16) as usize;
        let input_trunc = if input_display.len() > max_input {
            format!("{}…", &input_display[..max_input.saturating_sub(1)])
        } else {
            input_display
        };
        cmds.push(json!({"Text": {
            "x": 2 + prompt.len() as u16, "y": input_y,
            "text": input_trunc,
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
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

fn palette_commands() -> Vec<(String, String)> {
    vec![("Plugins".into(), "Irc Client".into())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("ctrl+up/down".into(), "switch channel".into()),
        ("esc".into(), "close".into()),
        ("enter".into(), "send message".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
        hints: key_hints(),
        palette_commands: palette_commands(),
        request: None,
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
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
                log::error!("[irc-client] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
