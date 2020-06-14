use semver::Version;
use std::result::Result;

pub fn serialize_version<S>(version: &Version, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{}", version))
}

pub fn deserialize_version<'de, D>(deserializer: D) -> Result<semver::Version, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Deserialize;
    let s = String::deserialize(deserializer)?;
    semver::Version::parse(&s).map_err(|err| serde::de::Error::custom(format!("{}", err)))
}
