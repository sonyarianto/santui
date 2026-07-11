use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[allow(dead_code)]
struct Element {
    number: u32,
    symbol: &'static str,
    name: &'static str,
    mass: f64,
    category: &'static str,
    group: u8,
    period: u8,
}

const ELEMENTS: &[Element] = &[
    Element {
        number: 1,
        symbol: "H",
        name: "Hydrogen",
        mass: 1.008,
        category: "nonmetal",
        group: 1,
        period: 1,
    },
    Element {
        number: 2,
        symbol: "He",
        name: "Helium",
        mass: 4.0026,
        category: "noble",
        group: 18,
        period: 1,
    },
    Element {
        number: 3,
        symbol: "Li",
        name: "Lithium",
        mass: 6.94,
        category: "alkali",
        group: 1,
        period: 2,
    },
    Element {
        number: 4,
        symbol: "Be",
        name: "Beryllium",
        mass: 9.0122,
        category: "alkaline",
        group: 2,
        period: 2,
    },
    Element {
        number: 5,
        symbol: "B",
        name: "Boron",
        mass: 10.81,
        category: "metalloid",
        group: 13,
        period: 2,
    },
    Element {
        number: 6,
        symbol: "C",
        name: "Carbon",
        mass: 12.011,
        category: "nonmetal",
        group: 14,
        period: 2,
    },
    Element {
        number: 7,
        symbol: "N",
        name: "Nitrogen",
        mass: 14.007,
        category: "nonmetal",
        group: 15,
        period: 2,
    },
    Element {
        number: 8,
        symbol: "O",
        name: "Oxygen",
        mass: 15.999,
        category: "nonmetal",
        group: 16,
        period: 2,
    },
    Element {
        number: 9,
        symbol: "F",
        name: "Fluorine",
        mass: 18.998,
        category: "halogen",
        group: 17,
        period: 2,
    },
    Element {
        number: 10,
        symbol: "Ne",
        name: "Neon",
        mass: 20.180,
        category: "noble",
        group: 18,
        period: 2,
    },
    Element {
        number: 11,
        symbol: "Na",
        name: "Sodium",
        mass: 22.990,
        category: "alkali",
        group: 1,
        period: 3,
    },
    Element {
        number: 12,
        symbol: "Mg",
        name: "Magnesium",
        mass: 24.305,
        category: "alkaline",
        group: 2,
        period: 3,
    },
    Element {
        number: 13,
        symbol: "Al",
        name: "Aluminium",
        mass: 26.982,
        category: "post-transition",
        group: 13,
        period: 3,
    },
    Element {
        number: 14,
        symbol: "Si",
        name: "Silicon",
        mass: 28.085,
        category: "metalloid",
        group: 14,
        period: 3,
    },
    Element {
        number: 15,
        symbol: "P",
        name: "Phosphorus",
        mass: 30.974,
        category: "nonmetal",
        group: 15,
        period: 3,
    },
    Element {
        number: 16,
        symbol: "S",
        name: "Sulfur",
        mass: 32.06,
        category: "nonmetal",
        group: 16,
        period: 3,
    },
    Element {
        number: 17,
        symbol: "Cl",
        name: "Chlorine",
        mass: 35.45,
        category: "halogen",
        group: 17,
        period: 3,
    },
    Element {
        number: 18,
        symbol: "Ar",
        name: "Argon",
        mass: 39.948,
        category: "noble",
        group: 18,
        period: 3,
    },
    Element {
        number: 19,
        symbol: "K",
        name: "Potassium",
        mass: 39.098,
        category: "alkali",
        group: 1,
        period: 4,
    },
    Element {
        number: 20,
        symbol: "Ca",
        name: "Calcium",
        mass: 40.078,
        category: "alkaline",
        group: 2,
        period: 4,
    },
    Element {
        number: 21,
        symbol: "Sc",
        name: "Scandium",
        mass: 44.956,
        category: "transition",
        group: 3,
        period: 4,
    },
    Element {
        number: 22,
        symbol: "Ti",
        name: "Titanium",
        mass: 47.867,
        category: "transition",
        group: 4,
        period: 4,
    },
    Element {
        number: 23,
        symbol: "V",
        name: "Vanadium",
        mass: 50.942,
        category: "transition",
        group: 5,
        period: 4,
    },
    Element {
        number: 24,
        symbol: "Cr",
        name: "Chromium",
        mass: 51.996,
        category: "transition",
        group: 6,
        period: 4,
    },
    Element {
        number: 25,
        symbol: "Mn",
        name: "Manganese",
        mass: 54.938,
        category: "transition",
        group: 7,
        period: 4,
    },
    Element {
        number: 26,
        symbol: "Fe",
        name: "Iron",
        mass: 55.845,
        category: "transition",
        group: 8,
        period: 4,
    },
    Element {
        number: 27,
        symbol: "Co",
        name: "Cobalt",
        mass: 58.933,
        category: "transition",
        group: 9,
        period: 4,
    },
    Element {
        number: 28,
        symbol: "Ni",
        name: "Nickel",
        mass: 58.693,
        category: "transition",
        group: 10,
        period: 4,
    },
    Element {
        number: 29,
        symbol: "Cu",
        name: "Copper",
        mass: 63.546,
        category: "transition",
        group: 11,
        period: 4,
    },
    Element {
        number: 30,
        symbol: "Zn",
        name: "Zinc",
        mass: 65.38,
        category: "transition",
        group: 12,
        period: 4,
    },
    Element {
        number: 31,
        symbol: "Ga",
        name: "Gallium",
        mass: 69.723,
        category: "post-transition",
        group: 13,
        period: 4,
    },
    Element {
        number: 32,
        symbol: "Ge",
        name: "Germanium",
        mass: 72.630,
        category: "metalloid",
        group: 14,
        period: 4,
    },
    Element {
        number: 33,
        symbol: "As",
        name: "Arsenic",
        mass: 74.922,
        category: "metalloid",
        group: 15,
        period: 4,
    },
    Element {
        number: 34,
        symbol: "Se",
        name: "Selenium",
        mass: 78.971,
        category: "nonmetal",
        group: 16,
        period: 4,
    },
    Element {
        number: 35,
        symbol: "Br",
        name: "Bromine",
        mass: 79.904,
        category: "halogen",
        group: 17,
        period: 4,
    },
    Element {
        number: 36,
        symbol: "Kr",
        name: "Krypton",
        mass: 83.798,
        category: "noble",
        group: 18,
        period: 4,
    },
    Element {
        number: 37,
        symbol: "Rb",
        name: "Rubidium",
        mass: 85.468,
        category: "alkali",
        group: 1,
        period: 5,
    },
    Element {
        number: 38,
        symbol: "Sr",
        name: "Strontium",
        mass: 87.62,
        category: "alkaline",
        group: 2,
        period: 5,
    },
    Element {
        number: 39,
        symbol: "Y",
        name: "Yttrium",
        mass: 88.906,
        category: "transition",
        group: 3,
        period: 5,
    },
    Element {
        number: 40,
        symbol: "Zr",
        name: "Zirconium",
        mass: 91.224,
        category: "transition",
        group: 4,
        period: 5,
    },
    Element {
        number: 41,
        symbol: "Nb",
        name: "Niobium",
        mass: 92.906,
        category: "transition",
        group: 5,
        period: 5,
    },
    Element {
        number: 42,
        symbol: "Mo",
        name: "Molybdenum",
        mass: 95.95,
        category: "transition",
        group: 6,
        period: 5,
    },
    Element {
        number: 43,
        symbol: "Tc",
        name: "Technetium",
        mass: 98.0,
        category: "transition",
        group: 7,
        period: 5,
    },
    Element {
        number: 44,
        symbol: "Ru",
        name: "Ruthenium",
        mass: 101.07,
        category: "transition",
        group: 8,
        period: 5,
    },
    Element {
        number: 45,
        symbol: "Rh",
        name: "Rhodium",
        mass: 102.91,
        category: "transition",
        group: 9,
        period: 5,
    },
    Element {
        number: 46,
        symbol: "Pd",
        name: "Palladium",
        mass: 106.42,
        category: "transition",
        group: 10,
        period: 5,
    },
    Element {
        number: 47,
        symbol: "Ag",
        name: "Silver",
        mass: 107.87,
        category: "transition",
        group: 11,
        period: 5,
    },
    Element {
        number: 48,
        symbol: "Cd",
        name: "Cadmium",
        mass: 112.41,
        category: "transition",
        group: 12,
        period: 5,
    },
    Element {
        number: 49,
        symbol: "In",
        name: "Indium",
        mass: 114.82,
        category: "post-transition",
        group: 13,
        period: 5,
    },
    Element {
        number: 50,
        symbol: "Sn",
        name: "Tin",
        mass: 118.71,
        category: "post-transition",
        group: 14,
        period: 5,
    },
    Element {
        number: 51,
        symbol: "Sb",
        name: "Antimony",
        mass: 121.76,
        category: "metalloid",
        group: 15,
        period: 5,
    },
    Element {
        number: 52,
        symbol: "Te",
        name: "Tellurium",
        mass: 127.60,
        category: "metalloid",
        group: 16,
        period: 5,
    },
    Element {
        number: 53,
        symbol: "I",
        name: "Iodine",
        mass: 126.90,
        category: "halogen",
        group: 17,
        period: 5,
    },
    Element {
        number: 54,
        symbol: "Xe",
        name: "Xenon",
        mass: 131.29,
        category: "noble",
        group: 18,
        period: 5,
    },
    Element {
        number: 55,
        symbol: "Cs",
        name: "Caesium",
        mass: 132.91,
        category: "alkali",
        group: 1,
        period: 6,
    },
    Element {
        number: 56,
        symbol: "Ba",
        name: "Barium",
        mass: 137.33,
        category: "alkaline",
        group: 2,
        period: 6,
    },
    Element {
        number: 57,
        symbol: "La",
        name: "Lanthanum",
        mass: 138.91,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 58,
        symbol: "Ce",
        name: "Cerium",
        mass: 140.12,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 59,
        symbol: "Pr",
        name: "Praseodymium",
        mass: 140.91,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 60,
        symbol: "Nd",
        name: "Neodymium",
        mass: 144.24,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 61,
        symbol: "Pm",
        name: "Promethium",
        mass: 145.0,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 62,
        symbol: "Sm",
        name: "Samarium",
        mass: 150.36,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 63,
        symbol: "Eu",
        name: "Europium",
        mass: 151.96,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 64,
        symbol: "Gd",
        name: "Gadolinium",
        mass: 157.25,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 65,
        symbol: "Tb",
        name: "Terbium",
        mass: 158.93,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 66,
        symbol: "Dy",
        name: "Dysprosium",
        mass: 162.50,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 67,
        symbol: "Ho",
        name: "Holmium",
        mass: 164.93,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 68,
        symbol: "Er",
        name: "Erbium",
        mass: 167.26,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 69,
        symbol: "Tm",
        name: "Thulium",
        mass: 168.93,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 70,
        symbol: "Yb",
        name: "Ytterbium",
        mass: 173.05,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 71,
        symbol: "Lu",
        name: "Lutetium",
        mass: 174.97,
        category: "lanthanide",
        group: 3,
        period: 6,
    },
    Element {
        number: 72,
        symbol: "Hf",
        name: "Hafnium",
        mass: 178.49,
        category: "transition",
        group: 4,
        period: 6,
    },
    Element {
        number: 73,
        symbol: "Ta",
        name: "Tantalum",
        mass: 180.95,
        category: "transition",
        group: 5,
        period: 6,
    },
    Element {
        number: 74,
        symbol: "W",
        name: "Tungsten",
        mass: 183.84,
        category: "transition",
        group: 6,
        period: 6,
    },
    Element {
        number: 75,
        symbol: "Re",
        name: "Rhenium",
        mass: 186.21,
        category: "transition",
        group: 7,
        period: 6,
    },
    Element {
        number: 76,
        symbol: "Os",
        name: "Osmium",
        mass: 190.23,
        category: "transition",
        group: 8,
        period: 6,
    },
    Element {
        number: 77,
        symbol: "Ir",
        name: "Iridium",
        mass: 192.22,
        category: "transition",
        group: 9,
        period: 6,
    },
    Element {
        number: 78,
        symbol: "Pt",
        name: "Platinum",
        mass: 195.08,
        category: "transition",
        group: 10,
        period: 6,
    },
    Element {
        number: 79,
        symbol: "Au",
        name: "Gold",
        mass: 196.97,
        category: "transition",
        group: 11,
        period: 6,
    },
    Element {
        number: 80,
        symbol: "Hg",
        name: "Mercury",
        mass: 200.59,
        category: "transition",
        group: 12,
        period: 6,
    },
    Element {
        number: 81,
        symbol: "Tl",
        name: "Thallium",
        mass: 204.38,
        category: "post-transition",
        group: 13,
        period: 6,
    },
    Element {
        number: 82,
        symbol: "Pb",
        name: "Lead",
        mass: 207.2,
        category: "post-transition",
        group: 14,
        period: 6,
    },
    Element {
        number: 83,
        symbol: "Bi",
        name: "Bismuth",
        mass: 208.98,
        category: "post-transition",
        group: 15,
        period: 6,
    },
    Element {
        number: 84,
        symbol: "Po",
        name: "Polonium",
        mass: 209.0,
        category: "post-transition",
        group: 16,
        period: 6,
    },
    Element {
        number: 85,
        symbol: "At",
        name: "Astatine",
        mass: 210.0,
        category: "halogen",
        group: 17,
        period: 6,
    },
    Element {
        number: 86,
        symbol: "Rn",
        name: "Radon",
        mass: 222.0,
        category: "noble",
        group: 18,
        period: 6,
    },
    Element {
        number: 87,
        symbol: "Fr",
        name: "Francium",
        mass: 223.0,
        category: "alkali",
        group: 1,
        period: 7,
    },
    Element {
        number: 88,
        symbol: "Ra",
        name: "Radium",
        mass: 226.0,
        category: "alkaline",
        group: 2,
        period: 7,
    },
    Element {
        number: 89,
        symbol: "Ac",
        name: "Actinium",
        mass: 227.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 90,
        symbol: "Th",
        name: "Thorium",
        mass: 232.04,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 91,
        symbol: "Pa",
        name: "Protactinium",
        mass: 231.04,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 92,
        symbol: "U",
        name: "Uranium",
        mass: 238.03,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 93,
        symbol: "Np",
        name: "Neptunium",
        mass: 237.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 94,
        symbol: "Pu",
        name: "Plutonium",
        mass: 244.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 95,
        symbol: "Am",
        name: "Americium",
        mass: 243.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 96,
        symbol: "Cm",
        name: "Curium",
        mass: 247.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 97,
        symbol: "Bk",
        name: "Berkelium",
        mass: 247.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 98,
        symbol: "Cf",
        name: "Californium",
        mass: 251.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 99,
        symbol: "Es",
        name: "Einsteinium",
        mass: 252.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 100,
        symbol: "Fm",
        name: "Fermium",
        mass: 257.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 101,
        symbol: "Md",
        name: "Mendelevium",
        mass: 258.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 102,
        symbol: "No",
        name: "Nobelium",
        mass: 259.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 103,
        symbol: "Lr",
        name: "Lawrencium",
        mass: 266.0,
        category: "actinide",
        group: 3,
        period: 7,
    },
    Element {
        number: 104,
        symbol: "Rf",
        name: "Rutherfordium",
        mass: 267.0,
        category: "transition",
        group: 4,
        period: 7,
    },
    Element {
        number: 105,
        symbol: "Db",
        name: "Dubnium",
        mass: 268.0,
        category: "transition",
        group: 5,
        period: 7,
    },
    Element {
        number: 106,
        symbol: "Sg",
        name: "Seaborgium",
        mass: 269.0,
        category: "transition",
        group: 6,
        period: 7,
    },
    Element {
        number: 107,
        symbol: "Bh",
        name: "Bohrium",
        mass: 270.0,
        category: "transition",
        group: 7,
        period: 7,
    },
    Element {
        number: 108,
        symbol: "Hs",
        name: "Hassium",
        mass: 269.0,
        category: "transition",
        group: 8,
        period: 7,
    },
    Element {
        number: 109,
        symbol: "Mt",
        name: "Meitnerium",
        mass: 278.0,
        category: "transition",
        group: 9,
        period: 7,
    },
    Element {
        number: 110,
        symbol: "Ds",
        name: "Darmstadtium",
        mass: 281.0,
        category: "transition",
        group: 10,
        period: 7,
    },
    Element {
        number: 111,
        symbol: "Rg",
        name: "Roentgenium",
        mass: 282.0,
        category: "transition",
        group: 11,
        period: 7,
    },
    Element {
        number: 112,
        symbol: "Cn",
        name: "Copernicium",
        mass: 285.0,
        category: "transition",
        group: 12,
        period: 7,
    },
    Element {
        number: 113,
        symbol: "Nh",
        name: "Nihonium",
        mass: 286.0,
        category: "post-transition",
        group: 13,
        period: 7,
    },
    Element {
        number: 114,
        symbol: "Fl",
        name: "Flerovium",
        mass: 289.0,
        category: "post-transition",
        group: 14,
        period: 7,
    },
    Element {
        number: 115,
        symbol: "Mc",
        name: "Moscovium",
        mass: 290.0,
        category: "post-transition",
        group: 15,
        period: 7,
    },
    Element {
        number: 116,
        symbol: "Lv",
        name: "Livermorium",
        mass: 293.0,
        category: "post-transition",
        group: 16,
        period: 7,
    },
    Element {
        number: 117,
        symbol: "Ts",
        name: "Tennessine",
        mass: 294.0,
        category: "halogen",
        group: 17,
        period: 7,
    },
    Element {
        number: 118,
        symbol: "Og",
        name: "Oganesson",
        mass: 294.0,
        category: "noble",
        group: 18,
        period: 7,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Search,
    List,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    query: String,
    selected: usize,
    focus: Focus,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            query: String::new(),
            selected: 0,
            focus: Focus::Search,
            status: "Type to search \u{b7} \u{2193} list \u{b7} enter detail \u{b7} c copy symbol"
                .into(),
        }
    }
}

impl App {
    fn filtered(&self) -> Vec<&'static Element> {
        let q = self.query.trim().to_lowercase();
        ELEMENTS
            .iter()
            .filter(|e| {
                q.is_empty()
                    || e.symbol.to_lowercase().contains(&q)
                    || e.name.to_lowercase().contains(&q)
                    || e.number.to_string() == q
                    || e.category.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn selected_element(&self) -> Option<&'static Element> {
        self.filtered().get(self.selected).copied()
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('c') if !modifiers.ctrl => {
                if let Some(e) = self.selected_element() {
                    match copy_to_clipboard(e.symbol) {
                        Ok(()) => self.status = format!("Copied {}", e.symbol),
                        Err(err) => self.status = format!("Clipboard error: {err}"),
                    }
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                if self.focus == Focus::Search {
                    let max = self.filtered().len().saturating_sub(1);
                    self.selected = self.selected.min(max).saturating_add(1).min(max);
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                if self.focus == Focus::Search {
                    self.selected = self.selected.saturating_sub(1);
                }
                true
            }
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Search => Focus::List,
                    Focus::List => Focus::Search,
                };
                true
            }
            IpcKey::Backspace => {
                if self.focus == Focus::Search {
                    self.query.pop();
                    self.selected = 0;
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                if self.focus == Focus::Search {
                    self.query.push(c);
                    self.selected = 0;
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(46);
    let h = app.area.h.max(16);

    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: t.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background_panel),
        title: Some(" Periodic Table ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!(
            "Search: {}",
            if app.query.is_empty() {
                "(all)"
            } else {
                &app.query
            }
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Search,
        modifiers: 0,
    });

    let list_y = 4;
    let list_h = h.saturating_sub(list_y + 2).max(1);
    let list_w = w.saturating_sub(4);

    let filtered = app.filtered();
    let start = app.selected.saturating_sub(list_h as usize / 2);
    let end = (start + list_h as usize).min(filtered.len());

    let items: Vec<String> = filtered[start..end]
        .iter()
        .map(|e| {
            format!(
                "{:>3}  {:<3} {:<14} {:>7.3}  {}",
                e.number, e.symbol, e.name, e.mass, e.category
            )
        })
        .collect();

    let vis_sel = if app.selected >= start && app.selected < end {
        Some(app.selected - start)
    } else {
        None
    };

    cmds.push(RenderCmd::List {
        x: 2,
        y: list_y,
        w: list_w,
        h: list_h,
        items,
        selected: vis_sel,
        style: TextStyle {
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(t.inverted_text),
            bg: Some(t.highlight),
            bold: true,
            modifiers: 0,
        },
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: "type search \u{b7} \u{2191}\u{2193}/jk navigate \u{b7} tab focus \u{b7} c copy \u{b7} esc".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
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
    vec![("Utilities".into(), "Open periodic table".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": [],
        "palette_commands": palette_commands(),
        "request": null,
        "plugin_message": null,
        "consumed": consumed,
    });
    if let Ok(json_str) = serde_json::to_string(&json) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{json_str}");
        let _ = out.flush();
    }
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
                log::error!("[periodic-table] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
