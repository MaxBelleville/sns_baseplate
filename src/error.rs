use std::fmt::{Display, Formatter};

/// Errors returned while configuring or sampling a baseplate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseplateError {
    /// One or more configuration problems found while building a baseplate.
    InvalidConfiguration(Vec<String>),
    /// An asset bundle or registry lookup required during loading failed.
    AssetLoad(String),
    /// A supplied world position contains NaN or infinity.
    InvalidSamplePosition,
    /// A required sample cell is not present in a cache.
    CacheCellMissing { x: i32, y: i32 },
    /// A required asset reference was not present at sampling time.
    MissingAsset(String),
    /// A generated or blended value became non-finite.
    NonFiniteSample(&'static str),
}

impl Display for BaseplateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfiguration(issues) => {
                write!(f, "invalid baseplate configuration: {}", issues.join("; "))
            }

            Self::AssetLoad(message) => write!(f, "baseplate asset loading failed: {message}"),

            Self::InvalidSamplePosition => write!(f, "sample position must be finite"),

            Self::CacheCellMissing { x, y } => {
                write!(f, "sample cache is missing grid cell ({x}, {y})")
            }

            Self::MissingAsset(message) => {
                write!(f, "required baseplate asset is missing: {message}")
            }

            Self::NonFiniteSample(name) => write!(f, "baseplate produced a non-finite {name}"),
        }
    }
}

impl std::error::Error for BaseplateError {}
