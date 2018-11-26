use core::fmt;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::profile::{Sha256Bytes, FileHash};

// Similar to GenericArray's provided serde code,
// but serializes to hex instead of an array.

impl Serialize for FileHash
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let as_hex = hex::encode(self.bytes.as_slice());
        serializer.serialize_str(&as_hex)
    }
}

struct FileHashVisitor;

impl<'de> Visitor<'de> for FileHashVisitor
{
    type Value = FileHash;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Byte array as hexadecimal")
    }

    fn visit_str<E>(self, s: &str) -> Result<FileHash, E>
    where E: de::Error
    {
        let decoded = hex::decode(s);
        match decoded {
            Ok(byte_vec) => {
                Ok(FileHash::new(Sha256Bytes::clone_from_slice(&byte_vec) ))
            },
            Err(invalid_hex) => {
                Err(match invalid_hex {
                    hex::FromHexError::InvalidHexCharacter{ c, index: _ } =>
                        de::Error::invalid_value(de::Unexpected::Char(c), &self),
                    _ => de::Error::invalid_length(s.len(), &self),
                })
            }
        }
    }
}

impl<'de> Deserialize<'de> for FileHash
{
    fn deserialize<D>(deserializer: D) -> Result<FileHash, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = FileHashVisitor;
        deserializer.deserialize_str(visitor)
    }
}
