use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Length,
    Mass,
    Temperature,
    Area,
    Volume,
    Time,
    Speed,
    Storage,
    Energy,
    Pressure,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::Mass => "Mass",
            Self::Temperature => "Temperature",
            Self::Area => "Area",
            Self::Volume => "Volume",
            Self::Time => "Time",
            Self::Speed => "Speed",
            Self::Storage => "Storage",
            Self::Energy => "Energy",
            Self::Pressure => "Pressure",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Unit {
    id: &'static str,
    label: &'static str,
    aliases: &'static [&'static str],
    category: Category,
    factor: f64,
}

const UNITS: &[Unit] = &[
    Unit {
        id: "m",
        label: "meter",
        aliases: &["metre", "meters"],
        category: Category::Length,
        factor: 1.0,
    },
    Unit {
        id: "km",
        label: "kilometer",
        aliases: &["kilometre", "kilometers"],
        category: Category::Length,
        factor: 1000.0,
    },
    Unit {
        id: "cm",
        label: "centimeter",
        aliases: &["centimetre", "centimeters"],
        category: Category::Length,
        factor: 0.01,
    },
    Unit {
        id: "mm",
        label: "millimeter",
        aliases: &["millimetre", "millimeters"],
        category: Category::Length,
        factor: 0.001,
    },
    Unit {
        id: "in",
        label: "inch",
        aliases: &["inches"],
        category: Category::Length,
        factor: 0.0254,
    },
    Unit {
        id: "ft",
        label: "foot",
        aliases: &["feet"],
        category: Category::Length,
        factor: 0.3048,
    },
    Unit {
        id: "yd",
        label: "yard",
        aliases: &["yards"],
        category: Category::Length,
        factor: 0.9144,
    },
    Unit {
        id: "mi",
        label: "mile",
        aliases: &["miles"],
        category: Category::Length,
        factor: 1609.344,
    },
    Unit {
        id: "kg",
        label: "kilogram",
        aliases: &["kilograms"],
        category: Category::Mass,
        factor: 1.0,
    },
    Unit {
        id: "g",
        label: "gram",
        aliases: &["grams"],
        category: Category::Mass,
        factor: 0.001,
    },
    Unit {
        id: "lb",
        label: "pound",
        aliases: &["pounds", "lbs"],
        category: Category::Mass,
        factor: 0.45359237,
    },
    Unit {
        id: "oz",
        label: "ounce",
        aliases: &["ounces"],
        category: Category::Mass,
        factor: 0.028349523125,
    },
    Unit {
        id: "c",
        label: "celsius",
        aliases: &["°c", "deg c"],
        category: Category::Temperature,
        factor: 1.0,
    },
    Unit {
        id: "f",
        label: "fahrenheit",
        aliases: &["°f", "deg f"],
        category: Category::Temperature,
        factor: 1.0,
    },
    Unit {
        id: "k",
        label: "kelvin",
        aliases: &["kelvins"],
        category: Category::Temperature,
        factor: 1.0,
    },
    Unit {
        id: "m2",
        label: "square meter",
        aliases: &["sqm"],
        category: Category::Area,
        factor: 1.0,
    },
    Unit {
        id: "km2",
        label: "square kilometer",
        aliases: &["sqkm"],
        category: Category::Area,
        factor: 1_000_000.0,
    },
    Unit {
        id: "ft2",
        label: "square foot",
        aliases: &["sqft"],
        category: Category::Area,
        factor: 0.09290304,
    },
    Unit {
        id: "ha",
        label: "hectare",
        aliases: &["hectares"],
        category: Category::Area,
        factor: 10_000.0,
    },
    Unit {
        id: "acre",
        label: "acre",
        aliases: &["acres"],
        category: Category::Area,
        factor: 4046.8564224,
    },
    Unit {
        id: "l",
        label: "liter",
        aliases: &["litre", "liters"],
        category: Category::Volume,
        factor: 1.0,
    },
    Unit {
        id: "ml",
        label: "milliliter",
        aliases: &["millilitre"],
        category: Category::Volume,
        factor: 0.001,
    },
    Unit {
        id: "m3",
        label: "cubic meter",
        aliases: &["cubic metre"],
        category: Category::Volume,
        factor: 1000.0,
    },
    Unit {
        id: "gal",
        label: "US gallon",
        aliases: &["gallon", "gallons"],
        category: Category::Volume,
        factor: 3.785411784,
    },
    Unit {
        id: "s",
        label: "second",
        aliases: &["sec", "seconds"],
        category: Category::Time,
        factor: 1.0,
    },
    Unit {
        id: "min",
        label: "minute",
        aliases: &["minutes"],
        category: Category::Time,
        factor: 60.0,
    },
    Unit {
        id: "h",
        label: "hour",
        aliases: &["hr", "hours"],
        category: Category::Time,
        factor: 3600.0,
    },
    Unit {
        id: "d",
        label: "day",
        aliases: &["days"],
        category: Category::Time,
        factor: 86400.0,
    },
    Unit {
        id: "mps",
        label: "meter/second",
        aliases: &["m/s"],
        category: Category::Speed,
        factor: 1.0,
    },
    Unit {
        id: "kph",
        label: "kilometer/hour",
        aliases: &["km/h", "kmh"],
        category: Category::Speed,
        factor: 1.0 / 3.6,
    },
    Unit {
        id: "mph",
        label: "mile/hour",
        aliases: &["mi/h"],
        category: Category::Speed,
        factor: 0.44704,
    },
    Unit {
        id: "b",
        label: "byte",
        aliases: &["bytes"],
        category: Category::Storage,
        factor: 1.0,
    },
    Unit {
        id: "kb",
        label: "kilobyte",
        aliases: &["KB"],
        category: Category::Storage,
        factor: 1000.0,
    },
    Unit {
        id: "kib",
        label: "kibibyte",
        aliases: &["KiB"],
        category: Category::Storage,
        factor: 1024.0,
    },
    Unit {
        id: "mb",
        label: "megabyte",
        aliases: &["MB"],
        category: Category::Storage,
        factor: 1_000_000.0,
    },
    Unit {
        id: "mib",
        label: "mebibyte",
        aliases: &["MiB"],
        category: Category::Storage,
        factor: 1_048_576.0,
    },
    Unit {
        id: "gb",
        label: "gigabyte",
        aliases: &["GB"],
        category: Category::Storage,
        factor: 1_000_000_000.0,
    },
    Unit {
        id: "gib",
        label: "gibibyte",
        aliases: &["GiB"],
        category: Category::Storage,
        factor: 1_073_741_824.0,
    },
    Unit {
        id: "j",
        label: "joule",
        aliases: &["joules"],
        category: Category::Energy,
        factor: 1.0,
    },
    Unit {
        id: "kj",
        label: "kilojoule",
        aliases: &["kilojoules"],
        category: Category::Energy,
        factor: 1000.0,
    },
    Unit {
        id: "kwh",
        label: "kilowatt-hour",
        aliases: &["kilowatt hour"],
        category: Category::Energy,
        factor: 3_600_000.0,
    },
    Unit {
        id: "cal",
        label: "calorie",
        aliases: &["calories"],
        category: Category::Energy,
        factor: 4.184,
    },
    Unit {
        id: "kcal",
        label: "kilocalorie",
        aliases: &["food calorie"],
        category: Category::Energy,
        factor: 4184.0,
    },
    Unit {
        id: "pa",
        label: "pascal",
        aliases: &["pascals"],
        category: Category::Pressure,
        factor: 1.0,
    },
    Unit {
        id: "kpa",
        label: "kilopascal",
        aliases: &["kilopascals"],
        category: Category::Pressure,
        factor: 1000.0,
    },
    Unit {
        id: "bar",
        label: "bar",
        aliases: &["bars"],
        category: Category::Pressure,
        factor: 100_000.0,
    },
    Unit {
        id: "psi",
        label: "pound/square inch",
        aliases: &["lb/in2"],
        category: Category::Pressure,
        factor: 6894.757293168,
    },
    Unit {
        id: "atm",
        label: "atmosphere",
        aliases: &["atmospheres"],
        category: Category::Pressure,
        factor: 101_325.0,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Amount,
    From,
    To,
    Search,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    amount: String,
    from_id: &'static str,
    to_id: &'static str,
    focus: Focus,
    query: String,
    selected: usize,
    precision: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            amount: "1".into(),
            from_id: "m",
            to_id: "ft",
            focus: Focus::Amount,
            query: String::new(),
            selected: 0,
            precision: 4,
            status: "Type amount · Tab focus · / search · s swap · c copy".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.next_focus();
                true
            }
            IpcKey::BackTab => {
                self.prev_focus();
                true
            }
            IpcKey::Char('/') => {
                self.focus = Focus::Search;
                true
            }
            IpcKey::Char('s') if !modifiers.ctrl => {
                std::mem::swap(&mut self.from_id, &mut self.to_id);
                self.status = "Swapped units".into();
                true
            }
            IpcKey::Char('+') => {
                self.precision = (self.precision + 1).min(10);
                true
            }
            IpcKey::Char('-') => {
                self.precision = self.precision.saturating_sub(1);
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_result();
                true
            }
            IpcKey::Backspace => {
                self.backspace();
                true
            }
            IpcKey::Char(c) if !c.is_control() => self.insert_char(c),
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.filtered_units().len().saturating_sub(1);
                self.selected = self.selected.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => self.select_unit(),
            IpcKey::Esc if !self.query.is_empty() || self.focus != Focus::Amount => {
                self.query.clear();
                self.focus = Focus::Amount;
                self.selected = 0;
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Amount => Focus::From,
            Focus::From => Focus::To,
            Focus::To => Focus::Search,
            Focus::Search => Focus::Amount,
        };
        self.selected = 0;
    }
    fn prev_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Amount => Focus::Search,
            Focus::From => Focus::Amount,
            Focus::To => Focus::From,
            Focus::Search => Focus::To,
        };
        self.selected = 0;
    }

    fn insert_char(&mut self, c: char) -> bool {
        match self.focus {
            Focus::Amount if c.is_ascii_digit() || matches!(c, '.' | '-' | '+') => {
                self.amount.push(c);
                true
            }
            Focus::Search => {
                self.query.push(c);
                self.selected = 0;
                true
            }
            _ => false,
        }
    }

    fn backspace(&mut self) {
        match self.focus {
            Focus::Amount => {
                self.amount.pop();
            }
            Focus::Search => {
                self.query.pop();
                self.selected = 0;
            }
            Focus::From | Focus::To => {}
        }
    }

    fn filtered_units(&self) -> Vec<&'static Unit> {
        let category = if self.focus == Focus::To {
            Some(unit_by_id(self.from_id).category)
        } else {
            None
        };
        filtered_units(&self.query, category)
    }

    fn select_unit(&mut self) -> bool {
        if !matches!(self.focus, Focus::From | Focus::To | Focus::Search) {
            return false;
        }
        let Some(unit) = self.filtered_units().get(self.selected).copied() else {
            return true;
        };
        if self.focus == Focus::To {
            self.to_id = unit.id;
            self.status = format!("Target set to {}", unit.label);
        } else {
            self.from_id = unit.id;
            if unit_by_id(self.to_id).category != unit.category {
                self.to_id = first_unit_in_category(unit.category).id;
            }
            self.status = format!("Source set to {}", unit.label);
        }
        true
    }

    fn copy_result(&mut self) {
        match self.result_line() {
            Ok(line) => match copy_to_clipboard(&line) {
                Ok(()) => self.status = "Copied conversion result".into(),
                Err(e) => self.status = format!("Clipboard error: {e}"),
            },
            Err(e) => self.status = e,
        }
    }

    fn result_line(&self) -> Result<String, String> {
        let value = parse_amount(&self.amount)?;
        let converted = convert(value, self.from_id, self.to_id)?;
        Ok(format!(
            "{} {} = {} {}",
            trim_float(value, self.precision),
            self.from_id,
            trim_float(converted, self.precision),
            self.to_id
        ))
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn filtered_units(query: &str, category: Option<Category>) -> Vec<&'static Unit> {
    let q = query.trim().to_lowercase();
    UNITS
        .iter()
        .filter(|u| category.is_none_or(|cat| u.category == cat))
        .filter(|u| {
            q.is_empty()
                || u.id.to_lowercase().contains(&q)
                || u.label.to_lowercase().contains(&q)
                || u.category.label().to_lowercase().contains(&q)
                || u.aliases.iter().any(|a| a.to_lowercase().contains(&q))
        })
        .collect()
}

fn unit_by_id(id: &str) -> &'static Unit {
    UNITS.iter().find(|u| u.id == id).unwrap_or(&UNITS[0])
}
fn first_unit_in_category(category: Category) -> &'static Unit {
    UNITS
        .iter()
        .find(|u| u.category == category)
        .unwrap_or(&UNITS[0])
}

fn parse_amount(input: &str) -> Result<f64, String> {
    let value = input
        .trim()
        .parse::<f64>()
        .map_err(|_| "Enter a valid number".to_string())?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Number must be finite".into())
    }
}

fn convert(value: f64, from_id: &str, to_id: &str) -> Result<f64, String> {
    let from = unit_by_id(from_id);
    let to = unit_by_id(to_id);
    if from.category != to.category {
        return Err(format!(
            "Cannot convert {} to {}",
            from.category.label(),
            to.category.label()
        ));
    }
    if from.category == Category::Temperature {
        let kelvin = match from.id {
            "c" => value + 273.15,
            "f" => (value - 32.0) * 5.0 / 9.0 + 273.15,
            "k" => value,
            _ => value,
        };
        return Ok(match to.id {
            "c" => kelvin - 273.15,
            "f" => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
            "k" => kelvin,
            _ => kelvin,
        });
    }
    Ok(value * from.factor / to.factor)
}

fn trim_float(value: f64, precision: usize) -> String {
    let mut s = format!("{value:.precision$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(40);
    let h = app.area.h.max(12);
    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        borders: BORDER_ALL,
        bg: Some(theme.background_panel),
        title: Some(" Unit Converter ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let from = unit_by_id(app.from_id);
    let to = unit_by_id(app.to_id);
    push_text(
        &mut cmds,
        2,
        2,
        focus_line(app.focus, Focus::Amount, "Amount", &app.amount),
        theme.text,
        app.focus == Focus::Amount,
    );
    push_text(
        &mut cmds,
        2,
        3,
        focus_line(
            app.focus,
            Focus::From,
            "From",
            &format!("{} ({})", from.label, from.id),
        ),
        theme.text,
        app.focus == Focus::From,
    );
    push_text(
        &mut cmds,
        2,
        4,
        focus_line(
            app.focus,
            Focus::To,
            "To",
            &format!("{} ({})", to.label, to.id),
        ),
        theme.text,
        app.focus == Focus::To,
    );
    push_text(
        &mut cmds,
        2,
        5,
        focus_line(app.focus, Focus::Search, "Search", &app.query),
        theme.text_muted,
        app.focus == Focus::Search,
    );

    match app.result_line() {
        Ok(result) => push_text(
            &mut cmds,
            2,
            7,
            format!("Result: {result}"),
            theme.success,
            true,
        ),
        Err(err) => push_text(&mut cmds, 2, 7, format!("Error: {err}"), theme.error, true),
    }

    let list_y = 9;
    let list_h = h.saturating_sub(list_y + 3).max(1);
    let units = app.filtered_units();
    let start = app.selected.saturating_sub(list_h as usize / 2);
    let items = units
        .iter()
        .skip(start)
        .take(list_h as usize)
        .map(|u| format!("{:<5} {:<22} {}", u.id, u.label, u.category.label()))
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: list_y,
        w: w.saturating_sub(4),
        h: list_h,
        items,
        selected: app.selected.checked_sub(start),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    });
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(2),
        &app.status,
        theme.text_muted,
        false,
    );
    cmds
}

fn focus_line(active: Focus, field: Focus, label: &str, value: &str) -> String {
    format!(
        "{} {label}: {value}",
        if active == field { ">" } else { " " }
    )
}

fn push_text(
    cmds: &mut Vec<RenderCmd>,
    x: u16,
    y: u16,
    text: impl Into<String>,
    fg: [u8; 3],
    bold: bool,
) {
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: text.into(),
        fg: Some(fg),
        bg: None,
        bold,
        modifiers: 0,
    });
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

fn hints() -> Vec<(String, String)> {
    vec![
        ("tab".into(), "focus".into()),
        ("enter".into(), "select".into()),
        ("s".into(), "swap".into()),
        ("+/-".into(), "precision".into()),
        ("c".into(), "copy".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![
        ("Utilities".into(), "Open unit converter".into()),
        ("Utilities".into(), "Swap unit conversion".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
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
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
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
            Ok(HostMsg::PaletteCommand { index }) => {
                if index == 1 {
                    std::mem::swap(&mut app.from_id, &mut app.to_id);
                }
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
                log::error!("[unit-converter] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn converts_length() {
        approx(convert(1.0, "mi", "ft").unwrap(), 5280.0);
    }

    #[test]
    fn converts_temperature() {
        approx(convert(32.0, "f", "c").unwrap(), 0.0);
        approx(convert(100.0, "c", "f").unwrap(), 212.0);
    }

    #[test]
    fn rejects_incompatible_categories() {
        assert!(convert(1.0, "m", "kg").is_err());
    }

    #[test]
    fn filters_by_alias_and_category() {
        assert_eq!(filtered_units("feet", None)[0].id, "ft");
        assert!(filtered_units("storage", None)
            .iter()
            .any(|u| u.id == "gib"));
    }

    #[test]
    fn trims_float_without_losing_precision() {
        assert_eq!(trim_float(1.2300, 4), "1.23");
        assert_eq!(trim_float(1.0, 4), "1");
    }
}
