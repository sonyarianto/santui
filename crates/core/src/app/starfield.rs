use std::time::Instant;

/// Starfield background animation, extracted from the monolithic Santui struct.
pub(super) struct Star {
    pub(super) x: u16,
    pub(super) y: u16,
    pub(super) phase: u16,
    pub(super) mag: u8,
    pub(super) freq: u16,
    pub(super) tint: u8,
}

pub(crate) struct Starfield {
    started_at: Instant,
    pub(super) stars: Vec<Star>,
}

fn star_count(width: u16, height: u16) -> usize {
    ((width as usize * height as usize) / 50).clamp(20, 200)
}

fn generate_stars(count: usize) -> Vec<Star> {
    let mut stars = Vec::with_capacity(count);
    let mut h = 0x9e3779b97f4a7c15u64;
    for _ in 0..count {
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
    stars
}

impl Default for Starfield {
    fn default() -> Self {
        Self::new()
    }
}

impl Starfield {
    pub(super) fn new() -> Self {
        Starfield {
            started_at: Instant::now(),
            stars: generate_stars(star_count(80, 24)),
        }
    }

    /// Time-based tick (1 tick = 100ms of wall-clock time).
    pub(super) fn tick(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64 / 100
    }

    /// Rebuild the starfield for a new terminal size.
    pub(super) fn resize(&mut self, width: u16, height: u16) {
        let count = star_count(width, height);
        if count != self.stars.len() {
            self.stars = generate_stars(count);
        }
    }
}
