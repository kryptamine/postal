/// A 24-bit colour, as mail and the terminal give it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
pub const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

impl Rgb {
    /// Relative luminance, 0 for black to 1 for white, as WCAG defines it.
    pub fn luminance(self) -> f32 {
        let linear = |channel: u8| {
            let c = f32::from(channel) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };

        0.0722f32.mul_add(
            linear(self.b),
            0.2126f32.mul_add(linear(self.r), 0.7152 * linear(self.g)),
        )
    }

    /// The WCAG contrast ratio, from 1 for the same colour to 21.
    pub fn contrast(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// `amount` of the way from `self` to `to`, `amount` in 0..=1.
    #[must_use]
    pub fn mix(self, to: Self, amount: f32) -> Self {
        let channel = |from: u8, to: u8| {
            let mixed = (f32::from(to) - f32::from(from)).mul_add(amount, f32::from(from));
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "rounded and clamped to 0..=255 first"
            )]
            let byte = mixed.round().clamp(0.0, 255.0) as u8;
            byte
        };

        Self {
            r: channel(self.r, to.r),
            g: channel(self.g, to.g),
            b: channel(self.b, to.b),
        }
    }

    /// How far apart two colours are, in their furthest channel.
    pub fn distance(self, other: Self) -> u8 {
        self.r
            .abs_diff(other.r)
            .max(self.g.abs_diff(other.g))
            .max(self.b.abs_diff(other.b))
    }

    /// How far the channels spread: low for a grey.
    pub fn chroma(self) -> u8 {
        let high = self.r.max(self.g).max(self.b);
        let low = self.r.min(self.g).min(self.b);
        high - low
    }
}
