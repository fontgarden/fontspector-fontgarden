//! The masters of a variable TrueType font.
//!
//! A master is a location where a gvar tuple has its peak, plus the default
//! location. At a master, a stored point is the default point plus the gvar
//! deltas. Points without an explicit delta get one by interpolation (IUP),
//! as the TrueType rasterizer does.

use std::collections::BTreeSet;

use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
    raw::{
        FontRef, ReadError, TableProvider,
        tables::glyf::Glyph,
        types::{F2Dot14, Tag},
    },
};

/// A master location, normalized and in user units.
#[derive(Clone, Debug)]
pub struct Master {
    pub normalized: Vec<F2Dot14>,
    pub user: Vec<(Tag, f32)>,
}

impl Master {
    pub fn describe(&self) -> String {
        let axes: Vec<String> = self
            .user
            .iter()
            .map(|(tag, value)| format!("{tag}={value}"))
            .collect();
        axes.join(", ")
    }
}

/// The default location and the peak of every gvar tuple.
pub fn masters(font: &FontRef) -> Result<Vec<Master>, ReadError> {
    let axis_count = font.axes().len();
    let mut peaks: BTreeSet<Vec<i16>> = BTreeSet::from([vec![0; axis_count]]);
    if let Ok(gvar) = font.gvar() {
        for gid in 0..font.maxp()?.num_glyphs() {
            if let Some(data) = gvar.glyph_variation_data(GlyphId::new(gid.into()))? {
                for tuple in data.tuples() {
                    peaks.insert(
                        tuple
                            .peak()
                            .values()
                            .iter()
                            .map(|v| v.get().to_bits())
                            .collect(),
                    );
                }
            }
        }
    }
    let segments = avar_segments(font, axis_count);
    Ok(peaks
        .into_iter()
        .map(|peak| {
            let normalized: Vec<F2Dot14> = peak.into_iter().map(F2Dot14::from_bits).collect();
            let user = font
                .axes()
                .iter()
                .zip(&normalized)
                .zip(&segments)
                .map(|((axis, value), map)| {
                    let n = invert_avar(map, value.to_f32());
                    let (min, default, max) =
                        (axis.min_value(), axis.default_value(), axis.max_value());
                    let span = if n > 0.0 {
                        max - default
                    } else {
                        default - min
                    };
                    (axis.tag(), default + n * span)
                })
                .collect();
            Master { normalized, user }
        })
        .collect())
}

/// The avar segment map of each axis as (from, to) pairs.
fn avar_segments(font: &FontRef, axis_count: usize) -> Vec<Vec<(f32, f32)>> {
    let mut maps = vec![vec![]; axis_count];
    if let Ok(avar) = font.avar() {
        for (map, segment) in maps
            .iter_mut()
            .zip(avar.axis_segment_maps().iter().flatten())
        {
            *map = segment
                .axis_value_maps()
                .iter()
                .map(|m| (m.from_coordinate().to_f32(), m.to_coordinate().to_f32()))
                .collect();
        }
    }
    maps
}

/// The normalized value that an avar segment map maps to `value`.
pub fn invert_avar(map: &[(f32, f32)], value: f32) -> f32 {
    for pair in map.windows(2) {
        let ((a, b), (c, d)) = (pair[0], pair[1]);
        if b <= value && value <= d {
            return if d == b {
                a
            } else {
                a + (value - b) * (c - a) / (d - b)
            };
        }
    }
    value
}

/// A stored point of a glyph at a master, with rounded coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredPoint {
    pub x: i32,
    pub y: i32,
    pub on_curve: bool,
}

/// The stored points of a simple glyph at a master. None for a composite or
/// empty glyph.
pub fn points(
    font: &FontRef,
    gid: GlyphId,
    master: &Master,
) -> Result<Option<Vec<StoredPoint>>, ReadError> {
    let (glyf, loca) = (font.glyf()?, font.loca(None)?);
    let Some(Glyph::Simple(glyph)) = loca.get_glyf(gid, &glyf)? else {
        return Ok(None);
    };
    let stored: Vec<_> = glyph.points().collect();
    let ends: Vec<usize> = glyph
        .end_pts_of_contours()
        .iter()
        .map(|e| e.get() as usize)
        .collect();
    let original: Vec<(f32, f32)> = stored.iter().map(|p| (p.x as f32, p.y as f32)).collect();
    let mut moved = original.clone();
    let at_default = master.normalized.iter().all(|v| v.to_bits() == 0);
    if !at_default
        && let Ok(gvar) = font.gvar()
        && let Some(data) = gvar.glyph_variation_data(gid)?
    {
        for tuple in data.tuples() {
            let scalar = tuple.compute_scalar_f32(&master.normalized).unwrap_or(0.0);
            if scalar == 0.0 {
                continue;
            }
            let mut explicit = vec![None; original.len()];
            for delta in tuple.deltas() {
                if let Some(slot) = explicit.get_mut(delta.position as usize) {
                    *slot = Some((delta.x_delta as f32, delta.y_delta as f32));
                }
            }
            for (point, delta) in moved.iter_mut().zip(iup(&original, &ends, &explicit)) {
                point.0 += scalar * delta.0;
                point.1 += scalar * delta.1;
            }
        }
    }
    Ok(Some(
        moved
            .iter()
            .zip(&stored)
            .map(|(&(x, y), p)| StoredPoint {
                x: round(x),
                y: round(y),
                on_curve: p.on_curve,
            })
            .collect(),
    ))
}

/// The advance width of a glyph at a master, rounded.
pub fn advance(font: &FontRef, gid: GlyphId, master: &Master) -> Option<i32> {
    font.glyph_metrics(Size::unscaled(), LocationRef::new(&master.normalized))
        .advance_width(gid)
        .map(round)
}

/// Round half up, as fontTools does.
pub fn round(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// Fill in the deltas of the points without an explicit delta, contour by
/// contour, by the TrueType interpolation of untouched points (IUP).
pub fn iup(
    original: &[(f32, f32)],
    ends: &[usize],
    explicit: &[Option<(f32, f32)>],
) -> Vec<(f32, f32)> {
    let mut out = vec![(0.0, 0.0); original.len()];
    let mut start = 0;
    for &end in ends {
        if end >= original.len() {
            break;
        }
        let range = start..end + 1;
        let touched: Vec<usize> = range.clone().filter(|&i| explicit[i].is_some()).collect();
        match touched.len() {
            0 => {}
            1 => {
                let delta = explicit[touched[0]].unwrap_or_default();
                for i in range.clone() {
                    out[i] = delta;
                }
            }
            _ => {
                for i in range.clone() {
                    if let Some(delta) = explicit[i] {
                        out[i] = delta;
                        continue;
                    }
                    // The touched points before and after i, around the contour.
                    let before = touched.iter().rev().find(|&&t| t < i).or(touched.last());
                    let after = touched.iter().find(|&&t| t > i).or(touched.first());
                    let (Some(&a), Some(&b)) = (before, after) else {
                        continue;
                    };
                    let (da, db) = (
                        explicit[a].unwrap_or_default(),
                        explicit[b].unwrap_or_default(),
                    );
                    out[i] = (
                        interpolate(original[a].0, original[b].0, da.0, db.0, original[i].0),
                        interpolate(original[a].1, original[b].1, da.1, db.1, original[i].1),
                    );
                }
            }
        }
        start = end + 1;
    }
    out
}

/// The IUP delta of one coordinate between two touched points.
fn interpolate(x1: f32, x2: f32, d1: f32, d2: f32, x: f32) -> f32 {
    let ((x1, d1), (x2, d2)) = if x1 <= x2 {
        ((x1, d1), (x2, d2))
    } else {
        ((x2, d2), (x1, d1))
    };
    if x1 == x2 {
        return if d1 == d2 { d1 } else { 0.0 };
    }
    if x <= x1 {
        d1
    } else if x >= x2 {
        d2
    } else {
        d1 + (x - x1) * (d2 - d1) / (x2 - x1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_touched_point_moves_the_contour() {
        let original = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let explicit = [None, Some((10.0, 4.0)), None];
        assert_eq!(iup(&original, &[2], &explicit), vec![(10.0, 4.0); 3]);
    }

    #[test]
    fn an_untouched_point_between_two_touched_points_is_interpolated() {
        let original = [(0.0, 0.0), (50.0, 0.0), (100.0, 0.0), (50.0, 100.0)];
        let explicit = [Some((0.0, 0.0)), None, Some((20.0, 0.0)), None];
        let out = iup(&original, &[3], &explicit);
        assert_eq!(out[1], (10.0, 0.0));
    }

    #[test]
    fn an_untouched_point_outside_the_touched_range_takes_the_nearer_delta() {
        let original = [(0.0, 0.0), (100.0, 0.0), (150.0, 0.0)];
        let explicit = [Some((2.0, 0.0)), Some((6.0, 0.0)), None];
        assert_eq!(iup(&original, &[2], &explicit)[2], (6.0, 0.0));
    }

    #[test]
    fn contours_are_interpolated_separately() {
        let original = [(0.0, 0.0), (10.0, 0.0), (0.0, 0.0), (10.0, 0.0)];
        let explicit = [Some((8.0, 0.0)), None, None, None];
        assert_eq!(
            iup(&original, &[1, 3], &explicit),
            vec![(8.0, 0.0), (8.0, 0.0), (0.0, 0.0), (0.0, 0.0)]
        );
    }

    #[test]
    fn avar_inversion_reverses_the_segment_map() {
        let map = [(-1.0, -1.0), (0.0, 0.0), (0.5, 0.25), (1.0, 1.0)];
        assert_eq!(invert_avar(&map, 0.25), 0.5);
        assert_eq!(invert_avar(&map, 1.0), 1.0);
        assert_eq!(invert_avar(&[], 0.3), 0.3);
    }

    #[test]
    fn rounding_is_half_up() {
        assert_eq!((round(1.5), round(-1.5), round(2.49)), (2, -1, 2));
    }
}
