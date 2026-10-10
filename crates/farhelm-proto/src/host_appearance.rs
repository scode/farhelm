//! Stable host-identity words shared by storage, HTTP validation and rendering.
//!
//! Choices are words rather than positions in a palette: adding or reordering
//! choices must never change a host's saved appearance. These are decorative
//! identities, independent of connection status and permission settings.
//!
//! Each word is spelled three times (serde's `rename_all` for HTTP and the UI,
//! `as_str` for database writes, `FromStr` for database reads), and the helm
//! refuses to list hosts at all when a stored word does not decode. The tests
//! below hold the three spellings together. Adding a word also needs a helm
//! schema bump: an older build at the same schema version would otherwise read
//! the new word from the database and fail every host listing.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// A remote host's chosen silhouette; cloud is the unset choice.
///
/// The same closed vocabulary validates HTTP input and decodes stored words;
/// unknown words are refused rather than mapped to a different identity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostIcon {
    #[default]
    Cloud,
    House,
    Flask,
    Database,
    Chip,
    Rocket,
    Gear,
    Gem,
    Hexagon,
    Triangle,
    Ring,
    Square,
    Bug,
    Factory,
    Castle,
}

impl HostIcon {
    /// Picker order only; persisted values never refer to these positions.
    pub const ALL: [Self; 15] = [
        Self::Cloud,
        Self::House,
        Self::Flask,
        Self::Database,
        Self::Chip,
        Self::Rocket,
        Self::Gear,
        Self::Gem,
        Self::Hexagon,
        Self::Triangle,
        Self::Ring,
        Self::Square,
        Self::Bug,
        Self::Factory,
        Self::Castle,
    ];

    /// The stable word used in the database and the public JSON fields.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::House => "house",
            Self::Flask => "flask",
            Self::Database => "database",
            Self::Chip => "chip",
            Self::Rocket => "rocket",
            Self::Gear => "gear",
            Self::Gem => "gem",
            Self::Hexagon => "hexagon",
            Self::Triangle => "triangle",
            Self::Ring => "ring",
            Self::Square => "square",
            Self::Bug => "bug",
            Self::Factory => "factory",
            Self::Castle => "castle",
        }
    }
}

impl FromStr for HostIcon {
    type Err = &'static str;

    /// Decode only the stable, case-sensitive words; unknown input is refused.
    fn from_str(word: &str) -> Result<Self, Self::Err> {
        match word {
            "cloud" => Ok(Self::Cloud),
            "house" => Ok(Self::House),
            "flask" => Ok(Self::Flask),
            "database" => Ok(Self::Database),
            "chip" => Ok(Self::Chip),
            "rocket" => Ok(Self::Rocket),
            "gear" => Ok(Self::Gear),
            "gem" => Ok(Self::Gem),
            "hexagon" => Ok(Self::Hexagon),
            "triangle" => Ok(Self::Triangle),
            "ring" => Ok(Self::Ring),
            "square" => Ok(Self::Square),
            "bug" => Ok(Self::Bug),
            "factory" => Ok(Self::Factory),
            "castle" => Ok(Self::Castle),
            _ => Err("unknown host icon"),
        }
    }
}

/// A host-identity tint, with default inheriting the ordinary foreground.
///
/// The same closed vocabulary validates HTTP input and decodes stored words;
/// unknown words are refused rather than mapped to a different identity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostColor {
    #[default]
    Default,
    Lavender,
    Orchid,
    Teal,
    Steel,
    Sand,
    Copper,
}

impl HostColor {
    /// Picker order only; persisted values never refer to these positions.
    pub const ALL: [Self; 7] = [
        Self::Default,
        Self::Lavender,
        Self::Orchid,
        Self::Teal,
        Self::Steel,
        Self::Sand,
        Self::Copper,
    ];

    /// The stable word used in the database and the public JSON fields.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Lavender => "lavender",
            Self::Orchid => "orchid",
            Self::Teal => "teal",
            Self::Steel => "steel",
            Self::Sand => "sand",
            Self::Copper => "copper",
        }
    }
}

impl FromStr for HostColor {
    type Err = &'static str;

    /// Decode only the stable, case-sensitive words; unknown input is refused.
    fn from_str(word: &str) -> Result<Self, Self::Err> {
        match word {
            "default" => Ok(Self::Default),
            "lavender" => Ok(Self::Lavender),
            "orchid" => Ok(Self::Orchid),
            "teal" => Ok(Self::Teal),
            "steel" => Ok(Self::Steel),
            "sand" => Ok(Self::Sand),
            "copper" => Ok(Self::Copper),
            _ => Err("unknown host color"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon word decodes back to its icon and matches the JSON spelling.
    ///
    /// Why: a stored word that `FromStr` cannot read makes the helm's whole
    /// host listing fail, and the only thing tying the three hand-written
    /// spellings together is this test.
    #[test]
    fn icon_words_round_trip_through_storage_and_json() {
        for icon in HostIcon::ALL {
            assert_eq!(icon.as_str().parse::<HostIcon>(), Ok(icon));
            let json = serde_json::to_string(&icon).expect("serialize icon");
            assert_eq!(json, format!("\"{}\"", icon.as_str()));
            assert_eq!(
                serde_json::from_str::<HostIcon>(&json).expect("deserialize icon"),
                icon
            );
        }
    }

    /// Every color word decodes back to its color and matches the JSON spelling.
    ///
    /// Why: as for icons, a stored color word the helm cannot decode fails
    /// every host listing, so storage and HTTP spellings must agree.
    #[test]
    fn color_words_round_trip_through_storage_and_json() {
        for color in HostColor::ALL {
            assert_eq!(color.as_str().parse::<HostColor>(), Ok(color));
            let json = serde_json::to_string(&color).expect("serialize color");
            assert_eq!(json, format!("\"{}\"", color.as_str()));
            assert_eq!(
                serde_json::from_str::<HostColor>(&json).expect("deserialize color"),
                color
            );
        }
    }
}
