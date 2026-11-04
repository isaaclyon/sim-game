//! Full-state saves (ADR 0033): a versioned envelope around the complete world.
//!
//! Layout, integers little-endian: the format tag `MAGIC`, the schema version (`u32`), the
//! payload length (`u64`), an FNV-1a checksum of the payload (`u64`), then the payload. The
//! payload encoding is private to this module; only derived caches and scratch are left out.

mod validate;

use std::fmt;

use super::World;

const MAGIC: &[u8; 8] = b"CITYSAVE";
/// Bump on any change to what the payload holds or how it is laid out. Older versions are
/// refused, never read as something else.
pub const SAVE_VERSION: u32 = 2;
const HEADER: usize = 8 + 4 + 8 + 8;

/// Why a save could not be written or read. Loading never yields a partial world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveError {
    /// The world could not be encoded.
    Encode(String),
    /// The bytes do not start with the save format tag.
    NotASave,
    /// A save of another schema version; this build reads only `SAVE_VERSION`.
    UnsupportedVersion(u32),
    /// The bytes end before the save does.
    Truncated,
    /// The payload does not match its length or checksum.
    Corrupt,
    /// The payload decodes to inconsistent state; names the first failed check.
    Invalid(&'static str),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "could not encode the city: {e}"),
            Self::NotASave => write!(f, "not a city save"),
            Self::UnsupportedVersion(v) => write!(
                f,
                "unsupported save version {v}; this build reads version {SAVE_VERSION}"
            ),
            Self::Truncated => write!(f, "the save is truncated"),
            Self::Corrupt => write!(f, "the save is corrupt"),
            Self::Invalid(what) => write!(f, "the save is inconsistent: {what}"),
        }
    }
}

impl std::error::Error for SaveError {}

fn checksum(bytes: &[u8]) -> u64 {
    // FNV-1a: every step is a bijection of the state, so any single changed byte is caught.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn le<const N: usize>(bytes: &[u8], at: usize) -> [u8; N] {
    bytes[at..at + N]
        .try_into()
        .expect("caller checked the length")
}

impl World {
    /// The complete simulation state as save bytes. Changes nothing.
    pub fn save(&self) -> Result<Vec<u8>, SaveError> {
        let payload = postcard::to_allocvec(self).map_err(|e| SaveError::Encode(e.to_string()))?;
        let mut out = Vec::with_capacity(HEADER + payload.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&SAVE_VERSION.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        out.extend_from_slice(&checksum(&payload).to_le_bytes());
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// The world `save` wrote. Every check runs before the world exists, so a failure leaves
    /// nothing half-loaded.
    pub fn load(bytes: &[u8]) -> Result<World, SaveError> {
        if bytes.get(..MAGIC.len()) != Some(MAGIC) {
            return Err(SaveError::NotASave);
        }
        if bytes.len() < 12 {
            return Err(SaveError::Truncated);
        }
        let version = u32::from_le_bytes(le(bytes, 8));
        if version != SAVE_VERSION {
            return Err(SaveError::UnsupportedVersion(version));
        }
        if bytes.len() < HEADER {
            return Err(SaveError::Truncated);
        }
        let len = u64::from_le_bytes(le(bytes, 12));
        let payload = &bytes[HEADER..];
        if (payload.len() as u64) < len {
            return Err(SaveError::Truncated);
        }
        if payload.len() as u64 != len || checksum(payload) != u64::from_le_bytes(le(bytes, 20)) {
            return Err(SaveError::Corrupt);
        }
        let (mut world, rest): (World, _) = postcard::take_from_bytes(payload)
            .map_err(|_| SaveError::Invalid("payload does not decode"))?;
        if !rest.is_empty() {
            return Err(SaveError::Invalid("payload has trailing bytes"));
        }
        world.validate().map_err(SaveError::Invalid)?;
        world.refresh_upkeep();
        Ok(world)
    }
}

#[cfg(test)]
mod tests;
