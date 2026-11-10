use super::Guid;
use crate::TagId;

/// What chunk 0's header says about the recording.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Header {
    /// The game build that recorded the film ("6.10026.18411.0"). Always
    /// set on a [`Film`](crate::Film): a header without one is refused with
    /// [`Error::UnknownHeader`](crate::Error::UnknownHeader).
    pub build: Option<String>,
    /// The build's branch ("HI_1_13_0").
    pub branch: Option<String>,
    /// When the film starts on the wall clock, Unix seconds. The only wall
    /// clock time a film holds.
    pub started_at: Option<u32>,
    /// The map variant's asset id.
    pub map_asset_id: Option<Guid>,
    /// The map variant's version id.
    pub map_version_id: Option<Guid>,
    /// The `levl` tag the map variant is built on:
    /// [`Hints::level`](crate::Hints::level) when given, else the one chunk 0
    /// names.
    pub level: Option<TagId>,
}
