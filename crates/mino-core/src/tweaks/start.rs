use crate::os::{BuildRange, WIN11_22H2};
use crate::provider::RegSpec;
use crate::tweak::{Category, Refresh, Tier, Tweak};
use crate::tweaks::helpers::{BoolTweak, ChoiceTweak};
use crate::tweaks::taskbar::ADVANCED;

/// Start reads these on next open, so nothing needs restarting.
const START_REFRESH: Refresh = Refresh::Broadcast("TraySettings");

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![
        Box::new(
            ChoiceTweak::new(
                "start.layout",
                Category::Start,
                RegSpec::hkcu(ADVANCED, "Start_Layout"),
                &[
                    ("default", 0),
                    ("more_pins", 1),
                    ("more_recommendations", 2),
                ],
                "default",
                START_REFRESH,
            )
            .tier(Tier::B)
            .builds(BuildRange::from(WIN11_22H2)),
        ),
        Box::new(
            BoolTweak::new(
                "start.recently_added_apps",
                Category::Start,
                RegSpec::hkcu(ADVANCED, "Start_TrackProgs"),
                START_REFRESH,
            )
            .default_on(true),
        ),
        Box::new(
            BoolTweak::new(
                "start.recommended_files",
                Category::Start,
                RegSpec::hkcu(ADVANCED, "Start_TrackDocs"),
                START_REFRESH,
            )
            .default_on(true),
        ),
    ]
}
