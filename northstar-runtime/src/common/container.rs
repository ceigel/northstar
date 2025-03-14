use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    convert::{TryFrom, TryInto},
    fmt::{self, Display},
    sync::Arc,
};
use thiserror::Error;

use super::{name::Name, version::Version};

/// Container identification
#[derive(Clone, Eq, PartialOrd, Ord, PartialEq, Debug, Hash)]
pub struct Container {
    inner: Arc<Inner>,
}

impl Container {
    /// Construct a new container
    pub fn new(name: Name, version: Version) -> Container {
        Container {
            inner: Arc::new(Inner { name, version }),
        }
    }

    /// Container name
    pub fn name(&self) -> &Name {
        &self.inner.name
    }

    /// Container version
    pub fn version(&self) -> &Version {
        &self.inner.version
    }
}

/// Parsing error for container identifier
#[derive(Error, Debug)]
#[error(transparent)]
pub struct InvalidContainerError(#[from] anyhow::Error);

impl Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.inner.name, self.inner.version,)
    }
}

impl TryFrom<&str> for Container {
    type Error = InvalidContainerError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (name, version) = value
            .split_once(':')
            .ok_or_else(|| anyhow!("missing container version"))?;
        let name = Name::try_from(name).context("invalid name")?;
        let version = Version::parse(version).context("invalid container version")?;
        Ok(Container::new(name, version))
    }
}

impl TryFrom<&Container> for Container {
    type Error = InvalidContainerError;

    fn try_from(container: &Container) -> Result<Self, Self::Error> {
        Ok(container.clone())
    }
}

impl<N, V> TryFrom<(N, V)> for Container
where
    N: TryInto<Name>,
    N::Error: Into<anyhow::Error>,
    V: ToString,
{
    type Error = InvalidContainerError;

    fn try_from((name, version): (N, V)) -> Result<Self, Self::Error> {
        let name = name
            .try_into()
            .map_err(Into::into)
            .context("invalid name")?;
        let version = Version::parse(&version.to_string()).context("invalid version")?;
        Ok(Container::new(name, version))
    }
}

impl Serialize for Container {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}:{}", self.inner.name, self.inner.version))
    }
}

impl<'de> Deserialize<'de> for Container {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Container::try_from(value.as_str()).map_err(serde::de::Error::custom)
    }
}

#[derive(Eq, PartialOrd, PartialEq, Ord, Debug, Hash, Serialize, Deserialize)]
struct Inner {
    name: Name,
    version: Version,
}

#[test]
#[allow(clippy::unwrap_used)]
fn try_from() {
    assert_eq!(
        Container::new("test".try_into().unwrap(), Version::parse("0.0.1").unwrap()),
        "test:0.0.1".try_into().unwrap()
    );
}

#[test]
fn invalid_name() {
    assert!(Container::try_from("test\0:0.0.1").is_err());
    assert!(Container::try_from("tes%t:0.0.1").is_err());
}
