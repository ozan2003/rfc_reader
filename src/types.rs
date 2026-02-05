//! Type aliases and common types used throughout the app.
use std::num::NonZeroU16;
use std::ops::Range;

/// Type alias for RFC numbers.
pub type RfcNum = NonZeroU16;

/// Type alias for line numbers.
pub type LineNumber = usize;

/// Type alias for matches spanning a line.
pub type MatchSpan = Range<usize>;
