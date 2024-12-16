use semver::{BuildMetadata, Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::digest::{
    consts::{B0, B1},
    generic_array::GenericArray,
    typenum::{UInt, UTerm},
};
use std::{
    error::Error,
    fmt::{Debug, Display},
    io::{self, Read, Write},
    path::StripPrefixError,
    str::{self, Utf8Error},
};
use thiserror::Error;

/// A SHA256 digest uniquely identifying a particular artifact's contents
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContentId([u8; 32]);
impl ContentId {
    pub fn unwrap(self) -> [u8; 32] {
        self.into()
    }
}
impl Debug for ContentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("sha256 [")?;
        for x in self.0 {
            f.write_fmt(format_args!("{:x}", x))?;
        }
        f.write_str(" ]")?;
        Ok(())
    }
}
impl Display for ContentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for x in self.0 {
            f.write_fmt(format_args!("{:x}", x))?;
        }
        Ok(())
    }
}
impl From<ContentId> for [u8; 32] {
    fn from(value: ContentId) -> Self {
        value.0
    }
}

// Conversion from sha2 crate's representation of a digest
type _FinalizedSha256 =
    GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>;
impl From<_FinalizedSha256> for ContentId {
    fn from(value: _FinalizedSha256) -> Self {
        let mut id = [0; 32];
        value.as_slice().read_exact(&mut id).unwrap();
        Self(id)
    }
}

// These allow us to use the hex feature of the serde_with crate
impl AsRef<[u8]> for ContentId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl TryFrom<Vec<u8>> for ContentId {
    type Error = ContentIdError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() != 32 {
            Err(ContentIdError::WrongWidth(value.len()))
        } else {
            let mut id = [0; 32];
            value.as_slice().read_exact(&mut id).unwrap();
            Ok(ContentId(id))
        }
    }
}

#[derive(Debug, Clone, Copy, Error)]
pub enum ContentIdError {
    #[error("Must be exactly 32 bytes (found {0} bytes)")]
    WrongWidth(usize),
}

/// An artifact represents built code which can be applied to a database.
pub trait Artifact {
    fn compatible(&self, version: &Version) -> bool;
    fn version(&self) -> &Version;
    fn spec(&self) -> (VersionReq, Version);
    fn scripts<C: ScriptConsumer>(
        &self,
        consumer: C,
    ) -> Result<ContentId, ScriptProcessingError<C::Error>>;
    fn write_to<F: Write>(
        &self,
        f: F,
    ) -> Result<ContentId, ScriptProcessingError<NullConsumerError>> {
        struct Consumer<W: Write> {
            f: W,
        }
        impl<W: Write> ScriptConsumer for Consumer<W> {
            type Error = NullConsumerError;

            fn accept(&mut self, script: &str) -> Result<(), ScriptProcessingError<Self::Error>> {
                self.f.write_all(script.as_bytes())?;
                Ok(())
            }

            fn commit(self, _id: ContentId) -> Result<(), ScriptProcessingError<Self::Error>> {
                Ok(())
            }
        }

        let consumer: Consumer<F> = Consumer { f };
        self.scripts(consumer)
    }
    fn content_id(&self) -> ContentId {
        struct Consumer;
        impl ScriptConsumer for Consumer {
            type Error = NullConsumerError;

            fn accept(&mut self, _script: &str) -> Result<(), ScriptProcessingError<Self::Error>> {
                Ok(())
            }

            fn commit(self, _id: ContentId) -> Result<(), ScriptProcessingError<Self::Error>> {
                Ok(())
            }
        }

        self.scripts(Consumer).unwrap()
    }
    fn to_string(&self) -> String {
        let mut bytes = Vec::with_capacity(1024);
        self.write_to(&mut bytes).unwrap();
        let s = str::from_utf8(&bytes).unwrap();

        s.to_string()
    }
}

/// Script consumers can access the code within an artifact.
pub trait ScriptConsumer {
    type Error: ConsumerError + Error + Debug + Sync + Send + 'static;
    fn accept(&mut self, script: &str) -> Result<(), ScriptProcessingError<Self::Error>>;
    fn commit(self, id: ContentId) -> Result<(), ScriptProcessingError<Self::Error>>;
}

pub trait ConsumerError {}

#[derive(Error, Debug)]
pub struct NullConsumerError;
impl Display for NullConsumerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("null error (should never occur)")
    }
}
impl ConsumerError for NullConsumerError {}

#[derive(Error, Debug)]
pub enum ScriptProcessingError<DatabaseError: Error + Debug> {
    #[error("The artifact is incompatible with this deployment")]
    Incompatible,

    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),

    #[error("Encoding error: {0}")]
    Utf8(#[from] Utf8Error),

    #[error("Could not process text: {0}")]
    Prefix(#[from] StripPrefixError),

    #[error("Database error: {0}")]
    Database(DatabaseError),

    #[error("Unknown error: {0}")]
    Other(#[from] anyhow::Error),
}
impl<C: Error + Debug + ConsumerError> From<C> for ScriptProcessingError<C> {
    fn from(value: C) -> Self {
        Self::Database(value)
    }
}
