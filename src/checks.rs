//! The Font.Garden checks.

use fontspector_checkapi::{FileTypeConvert, prelude::*, skip, testfont};
use skrifa::{GlyphId, raw::TableProvider, string::StringId};

use crate::masters;

pub const UNITS_PER_EM: u16 = 1024;
pub const GRID: i32 = 2;
pub const LICENSE: &str = "SIL Open Font License, Version 1.1";
pub const LICENSE_URL: &str = "https://openfontlicense.org";

#[check(
    id = "fontgarden/unitsperem",
    title = "The em is 1024 units",
    rationale = "The Font.Garden grid divides the em by powers of two. 1024 is 2 to the
        power 10, so the grids of 2, 8 and 64 units and the vertical metrics are whole
        numbers.",
    proposal = "https://elih.net/blog/virtua-grotesk/"
)]
fn units_per_em(t: &Testable, _context: &Context) -> CheckFnResult {
    let font = testfont!(t);
    let upm = font.font().head()?.units_per_em();
    Ok(if upm == UNITS_PER_EM {
        Status::just_one_pass()
    } else {
        Status::just_one_fail(
            "wrong-upm",
            &format!("unitsPerEm is {upm}, not {UNITS_PER_EM}."),
        )
    })
}

#[check(
    id = "fontgarden/license",
    title = "The name table gives the OFL 1.1",
    rationale = "Font.Garden fonts are licensed under the SIL Open Font License 1.1.
        Name ID 13 gives the license and name ID 14 its URL.",
    proposal = "https://openfontlicense.org"
)]
fn license(t: &Testable, _context: &Context) -> CheckFnResult {
    let font = testfont!(t);
    let mut problems = vec![];
    let descriptions: Vec<String> = font
        .get_name_entry_strings(StringId::LICENSE_DESCRIPTION)
        .collect();
    if descriptions.is_empty() || descriptions.iter().any(|d| !d.contains(LICENSE)) {
        problems.push(Status::fail(
            "license-description",
            &format!("Name ID 13 does not contain \"{LICENSE}\"."),
        ));
    }
    let urls: Vec<String> = font.get_name_entry_strings(StringId::LICENSE_URL).collect();
    if urls.is_empty() || urls.iter().any(|u| u != LICENSE_URL) {
        problems.push(Status::fail(
            "license-url",
            &format!("Name ID 14 is {urls:?}, not {LICENSE_URL}."),
        ));
    }
    return_result(problems)
}

#[check(
    id = "fontgarden/grid",
    title = "The masters are on the 2-unit grid",
    rationale = "Every point of a Font.Garden source is on the 2-unit grid, so that a
        coordinate tells its grid level. The conversion to quadratic curves moves the
        off-curve points, so this check reads the on-curve points and the advance widths
        of every master of the variable font. The coordinates are rounded first: an
        optimized gvar table can put a master up to half a unit off its source. Static
        fonts are instances, mostly between masters, so they are not checked.",
    proposal = "https://elih.net/blog/virtua-grotesk/"
)]
fn grid(t: &Testable, _context: &Context) -> CheckFnResult {
    let font = testfont!(t);
    skip!(
        !font.is_variable_font(),
        "static",
        "A static font is an instance; the masters are checked in the variable font."
    );
    let reader = font.font();
    let mut problems = vec![];
    for master in masters::masters(&reader)? {
        let mut found = vec![];
        for gid in font.all_glyphs() {
            if let Some(problem) = off_grid(&reader, gid, &master)? {
                found.push(format!(
                    "- {}: {problem}",
                    font.glyph_name_for_id_synthesise(gid)
                ));
            }
        }
        if !found.is_empty() {
            problems.push(Status::fail(
                "off-grid",
                &format!(
                    "At {}, {} glyphs are off the {GRID}-unit grid:\n{}",
                    master.describe(),
                    found.len(),
                    found.join("\n")
                ),
            ));
        }
    }
    return_result(problems)
}

/// The on-curve points and the advance of a glyph that are off the grid.
fn off_grid(
    font: &skrifa::FontRef,
    gid: GlyphId,
    master: &masters::Master,
) -> Result<Option<String>, FontspectorError> {
    let mut parts = vec![];
    if let Some(advance) = masters::advance(font, gid, master)
        && advance % GRID != 0
    {
        parts.push(format!("advance {advance}"));
    }
    if let Some(points) = masters::points(font, gid, master)? {
        let odd: Vec<_> = points
            .iter()
            .filter(|p| p.on_curve && (p.x % GRID != 0 || p.y % GRID != 0))
            .collect();
        if let Some(p) = odd.first() {
            parts.push(format!(
                "{} points, for example ({}, {})",
                odd.len(),
                p.x,
                p.y
            ));
        }
    }
    Ok((!parts.is_empty()).then(|| parts.join(", ")))
}
