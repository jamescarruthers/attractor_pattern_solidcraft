//! Colours: hex, RGB and HSL to RGB in 0..1, and back to the "#rrggbb" the engine takes.

use crate::params::Colour;

pub fn rgb(c: &Colour) -> Result<[f64; 3], String> {
    match c {
        Colour::Hex(s) => hex(s),
        Colour::Rgb(v) => {
            if v.iter().all(|x| x.is_finite() && (0.0..=255.0).contains(x)) {
                Ok(v.map(|x| x / 255.0))
            } else {
                Err("RGB colours take three numbers from 0 to 255".into())
            }
        }
        Colour::Hsl { h, s, l } => {
            if [h, s, l].iter().all(|x| x.is_finite()) && (0.0..=1.0).contains(s) && (0.0..=1.0).contains(l) {
                Ok(hsl(*h, *s, *l))
            } else {
                Err("HSL colours take h in degrees and s and l from 0 to 1".into())
            }
        }
    }
}

fn hex(s: &str) -> Result<[f64; 3], String> {
    let t = s.trim().trim_start_matches('#');
    let bad = || format!("`{s}` is not a colour like #RRGGBB");
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let byte = |i: usize| t.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok()).map(|b| f64::from(b) / 255.0).ok_or_else(bad);
    Ok([byte(0)?, byte(2)?, byte(4)?])
}

fn hsl(h: f64, s: f64, l: f64) -> [f64; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r + m, g + m, b + m]
}

/// "#rrggbb" from channels in 0..1 (clamped).
pub fn to_hex(c: [f64; 3]) -> String {
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_agree() {
        assert_eq!(to_hex(rgb(&Colour::Hex("#00F0F0".into())).unwrap()), "#00f0f0");
        assert_eq!(to_hex(rgb(&Colour::Rgb([255.0, 128.0, 0.0])).unwrap()), "#ff8000");
        assert_eq!(to_hex(rgb(&Colour::Hsl { h: 0.0, s: 1.0, l: 0.5 }).unwrap()), "#ff0000");
        assert_eq!(to_hex(rgb(&Colour::Hsl { h: 240.0, s: 1.0, l: 0.5 }).unwrap()), "#0000ff");
        assert!(rgb(&Colour::Hex("12345".into())).is_err());
    }
}
