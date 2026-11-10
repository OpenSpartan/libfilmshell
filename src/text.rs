//! Ids written as text: their `Display` form, read back with `FromStr`.

use std::fmt;

/// An id string that doesn't parse: [`TagId`](crate::TagId), [`Xuid`](crate::Xuid)
/// or [`Guid`](crate::Guid) from text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseIdError {
    pub(crate) kind: &'static str,
}

impl fmt::Display for ParseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a {}", self.kind)
    }
}

impl std::error::Error for ParseIdError {}

/// Serialize as the type's `Display` text and deserialize through `FromStr`.
macro_rules! serde_as_text {
    ($t:ty) => {
        #[cfg(feature = "serde")]
        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <String as serde::Deserialize>::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(crate) use serde_as_text;
