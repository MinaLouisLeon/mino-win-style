use crate::os::{BuildRange, WIN11_22H2, WIN11_24H2};
use crate::provider::RegSpec;
use crate::tweak::{Category, Refresh, Tier, Tweak};
use crate::tweaks::helpers::{BoolTweak, ChoiceTweak};

pub(crate) const ADVANCED: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";

/// Explorer watches these keys directly and picks most of them up within a
/// moment, but we still broadcast so anything else listening keeps in step.
const TRAY: Refresh = Refresh::Broadcast("TraySettings");

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![
        Box::new(ChoiceTweak::new(
            "taskbar.alignment",
            Category::Taskbar,
            RegSpec::hkcu(ADVANCED, "TaskbarAl"),
            &[("left", 0), ("center", 1)],
            "center",
            TRAY,
        )),
        Box::new(
            BoolTweak::new(
                "taskbar.widgets",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "TaskbarDa"),
                TRAY,
            )
            .default_on(true)
            .tier(Tier::B),
        ),
        Box::new(
            BoolTweak::new(
                "taskbar.task_view",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "ShowTaskViewButton"),
                TRAY,
            )
            .default_on(true),
        ),
        Box::new(ChoiceTweak::new(
            "taskbar.search",
            Category::Taskbar,
            RegSpec::hkcu(SEARCH, "SearchboxTaskbarMode"),
            &[
                ("hidden", 0),
                ("icon", 1),
                ("box", 2),
                ("icon_and_label", 3),
            ],
            "box",
            TRAY,
        )),
        Box::new(
            ChoiceTweak::new(
                "taskbar.icon_size",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "TaskbarSi"),
                &[("small", 0), ("medium", 1), ("large", 2)],
                "medium",
                TRAY,
            )
            .tier(Tier::B)
            // Reliable up to 22H2; later builds ignore it or handle it
            // inconsistently, so it is gated rather than quietly broken.
            .builds(BuildRange::between(
                crate::os::WIN11_21H2,
                WIN11_22H2,
            )),
        ),
        Box::new(
            BoolTweak::new(
                "taskbar.seconds_in_clock",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "ShowSecondsInSystemClock"),
                TRAY,
            )
            .builds(BuildRange::from(WIN11_22H2)),
        ),
        Box::new(
            BoolTweak::new(
                "taskbar.end_task",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "TaskbarEndTask"),
                TRAY,
            )
            .builds(BuildRange::from(WIN11_24H2))
            .note("Mirrors the End Task switch in Settings for developers."),
        ),
    ]
}
