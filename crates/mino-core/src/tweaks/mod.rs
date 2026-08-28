//! Every setting the app knows about.
//!
//! Adding one means adding a literal to the relevant `all()` below. If it needs
//! more than [`helpers::BoolTweak`] or [`helpers::ChoiceTweak`] can express,
//! write a type that implements [`Tweak`] and keep the key paths inside it.

pub mod appearance;
pub mod explorer;
pub mod helpers;
pub mod start;
pub mod taskbar;

use crate::tweak::Tweak;

pub struct TweakRegistry {
    tweaks: Vec<Box<dyn Tweak>>,
}

impl TweakRegistry {
    pub fn builtin() -> Self {
        let mut tweaks: Vec<Box<dyn Tweak>> = Vec::new();
        tweaks.extend(appearance::all());
        tweaks.extend(taskbar::all());
        tweaks.extend(start::all());
        tweaks.extend(explorer::all());
        TweakRegistry { tweaks }
    }

    pub fn get(&self, id: &str) -> Option<&dyn Tweak> {
        self.tweaks
            .iter()
            .find(|t| t.id() == id)
            .map(|t| t.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn Tweak> {
        self.tweaks.iter().map(|t| t.as_ref())
    }

    pub fn len(&self) -> usize {
        self.tweaks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tweaks.is_empty()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.tweaks.iter().map(|t| t.id()).collect()
    }
}

impl Default for TweakRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique() {
        let registry = TweakRegistry::builtin();
        let unique: BTreeSet<&str> = registry.ids().into_iter().collect();
        assert_eq!(unique.len(), registry.len(), "duplicate tweak id");
    }

    #[test]
    fn ids_are_prefixed_with_their_category() {
        for tweak in TweakRegistry::builtin().iter() {
            let prefix = format!("{}.", tweak.category().id());
            assert!(
                tweak.id().starts_with(&prefix),
                "`{}` should start with `{prefix}`",
                tweak.id()
            );
        }
    }

    #[test]
    fn no_tier_c_ships_in_this_binary() {
        for tweak in TweakRegistry::builtin().iter() {
            assert_ne!(
                tweak.tier(),
                crate::tweak::Tier::C,
                "`{}` is Tier C — that belongs in a plugin, not here",
                tweak.id()
            );
        }
    }
}
