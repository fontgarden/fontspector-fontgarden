//! Fontspector plugin with the Font.Garden profile.
//!
//! The profile `fontgarden` is the Google Fonts profile with the Font.Garden
//! requirements added. A requirement that an existing Fontspector check can
//! test is configured here. A new requirement is a check in `checks.rs`.

pub mod checks;
pub mod masters;

use std::collections::HashMap;

use fontspector_checkapi::{FontspectorError, ProfileBuilder, Registry};
use serde_json::json;

/// The Font.Garden vendor ID, registered with Microsoft.
pub const VENDOR_ID: &str = "FTGD";

pub struct FontGarden;

impl fontspector_checkapi::ProfileProvider for FontGarden {
    fn register(&self, registry: &mut Registry) -> Result<(), FontspectorError> {
        ProfileBuilder::new()
            .include_profile("googlefonts")
            // An included profile does not bring its exclusions and
            // configuration. These are copied from the googlefonts profile of
            // Fontspector 1.9.
            .exclude_check("cmap/format_12")
            .exclude_check("empty_letters")
            .exclude_check("inconsistencies_between_fvar_STAT")
            .exclude_check("no_mac_entries")
            .exclude_check("typographic_family_name")
            .exclude_check("vtt_volt_data")
            .with_configuration_defaults(
                "file_size",
                HashMap::from([
                    ("WARN_SIZE".to_string(), json!(1_048_576)),
                    ("FAIL_SIZE".to_string(), json!(9_437_184)),
                    ("FATAL_SIZE".to_string(), json!(10_485_760)),
                ]),
            )
            .with_configuration_defaults(
                "googlefonts/family/file_size",
                HashMap::from([("FATAL_SIZE".to_string(), json!(26_214_400))]),
            )
            // Font.Garden requirements.
            .with_configuration_defaults(
                "opentype/vendor_id",
                HashMap::from([("vendor_id".to_string(), json!(VENDOR_ID))]),
            )
            .add_section("Font.Garden")
            .add_and_register_check(checks::units_per_em)
            .add_and_register_check(checks::grid)
            .add_and_register_check(checks::license)
            .build("fontgarden", registry)
    }
}
