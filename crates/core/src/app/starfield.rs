/// Starfield background animation, extracted from the monolithic Santui struct.
pub(super) struct Star {
    pub(super) x: u16,
    pub(super) y: u16,
    pub(super) phase: u16,
    pub(super) mag: u8,
    pub(super) freq: u16,
    pub(super) tint: u8,
}

pub(super) struct ShootingStar {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) dx: f64,
    pub(super) dy: f64,
    pub(super) age: u64,
    pub(super) kind: u8,
}

pub(crate) struct Starfield {
    pub(super) tick: u64,
    pub(super) stars: Vec<Star>,
    pub(super) shooting: Option<ShootingStar>,
    shooting_cooldown: u64,
}

const STAR_COUNT: usize = 88;
const SHOOTING_LIFETIME: u64 = 50;
const SHOOTING_COOLDOWN: u64 = 180;
const COMET_LIFETIME: u64 = 100;
const COMET_COOLDOWN: u64 = 500;

impl Starfield {
    pub(super) fn new() -> Self {
        let mut stars = Vec::with_capacity(STAR_COUNT);
        let mut h = 0x9e3779b97f4a7c15u64;
        for _ in 0..STAR_COUNT {
            h = h
                .wrapping_mul(0x5851f42d4c957f2d)
                .wrapping_add(0x14057b7ef767814f);
            let a = h >> 32;
            let b = h >> 16;
            let c = h;
            stars.push(Star {
                x: (a % 1009) as u16,
                y: (b % 1009) as u16,
                phase: (c % 628) as u16,
                mag: {
                    let m = ((c >> 8) & 0xff) as u8;
                    if m < 100 {
                        0
                    } else if m < 200 {
                        1
                    } else if m < 240 {
                        2
                    } else {
                        3
                    }
                },
                freq: (4 + ((c >> 12) & 0x3f)) as u16,
                tint: (c >> 20 & 0xff) as u8,
            });
        }
        Starfield {
            tick: 0,
            stars,
            shooting: None,
            shooting_cooldown: 0,
        }
    }

    pub(super) fn update(&mut self) {
        let n = (self.tick ^ 0xdeadbeef)
            .wrapping_mul(1103515245)
            .wrapping_add(12345);
        let r = n >> 16;
        self.shooting_cooldown = self.shooting_cooldown.saturating_sub(1);
        let kind = if (r & 0x80) == 0 { 0 } else { 1 };
        if self.shooting.is_none() && self.shooting_cooldown == 0 && (r & 0x3f) < 6 {
            let (speed, max_extra) = if kind == 0 { (1.0, 1.2) } else { (0.2, 0.4) };
            let side = r & 3;
            let (x, y, dx, dy) = match side {
                0 => (
                    0.0,
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    speed + ((r >> 12 & 0x7f) as f64 / 256.0) * max_extra,
                    speed * 0.6 + ((r >> 19 & 0x7f) as f64 / 256.0) * max_extra,
                ),
                1 => (
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    0.0,
                    speed * 0.5 + ((r >> 12 & 0x7f) as f64 / 256.0) * max_extra,
                    speed + ((r >> 19 & 0x7f) as f64 / 256.0) * max_extra,
                ),
                _ => (
                    1.0,
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    -speed - ((r >> 12 & 0x7f) as f64 / 256.0) * max_extra,
                    speed * 0.6 + ((r >> 19 & 0x7f) as f64 / 256.0) * max_extra,
                ),
            };
            self.shooting = Some(ShootingStar {
                x,
                y,
                dx,
                dy,
                age: 0,
                kind,
            });
        }
        if let Some(ref mut s) = self.shooting {
            let speed = if s.kind == 0 { 100.0 } else { 180.0 };
            s.x += s.dx / speed;
            s.y += s.dy / speed;
            s.age += 1;
        }
        let shooting_expired = self.shooting.as_ref().is_some_and(|s| {
            let max_age = if s.kind == 0 {
                SHOOTING_LIFETIME
            } else {
                COMET_LIFETIME
            };
            s.age > max_age || s.x < -0.3 || s.x > 1.3 || s.y > 1.3
        });
        if shooting_expired {
            let kind = self.shooting.as_ref().map(|s| s.kind).unwrap_or(0);
            let cooldown = if kind == 0 {
                SHOOTING_COOLDOWN
            } else {
                COMET_COOLDOWN
            };
            self.shooting = None;
            self.shooting_cooldown = cooldown + (r & 0xff);
        }
    }
}
