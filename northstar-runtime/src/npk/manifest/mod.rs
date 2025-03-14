use crate::{
    common::{container::Container, name::Name, non_nul_string::NonNulString, version::Version},
    seccomp::Seccomp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{
    rust::{maps_duplicate_key_is_error, sets_duplicate_value_is_error},
    skip_serializing_none,
};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use thiserror::Error;
use validator::{Validate, ValidationErrors};

use self::network::Network;

/// Autostart
pub mod autostart;
/// Linux capabilities
pub mod capabilities;
/// Linux control groups
pub mod cgroups;
/// Northstar console configuration
pub mod console;
/// Container io
pub mod io;
/// Container mounts
pub mod mount;
/// Networking
pub mod network;
/// Linux resource limits
pub mod rlimit;
/// SE Linux
pub mod selinux;

mod validation;

/// Manifest parsing error
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("invalid manifest: {0}")]
    Validation(ValidationErrors),
    #[error("failed to parse: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Northstar package manifest
#[skip_serializing_none]
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validation::manifest"))]
pub struct Manifest {
    /// Name of container
    pub name: Name,
    /// Container version
    pub version: Version,
    /// Pass a console fd number in NORTHSTAR_CONSOLE
    pub console: Option<console::Configuration>,
    /// Path to init
    #[validate(custom = "validation::init")]
    pub init: Option<NonNulString>,
    /// Additional arguments for the application invocation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<NonNulString>,
    /// Environment passed to container
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[validate(custom = "validation::env")]
    pub env: HashMap<NonNulString, NonNulString>,
    /// UID
    #[validate(range(min = 1, message = "uid must be greater than 0"))]
    pub uid: u16,
    /// GID
    #[validate(range(min = 1, message = "gid must be greater than 0"))]
    pub gid: u16,
    /// List of bind mounts and resources
    #[serde(
        default,
        skip_serializing_if = "HashMap::is_empty",
        deserialize_with = "maps_duplicate_key_is_error::deserialize"
    )]
    #[validate(custom = "validation::mounts")]
    pub mounts: HashMap<mount::MountPoint, mount::Mount>,
    /// Autostart this container upon northstar startup
    pub autostart: Option<autostart::Autostart>,
    /// CGroup configuration
    pub cgroups: Option<self::cgroups::CGroups>,
    /// Network configuration. Unshare the network if omitted.
    #[validate(custom = "validation::network")]
    pub network: Option<Network>,
    /// Seccomp configuration
    #[validate(custom = "validation::seccomp")]
    pub seccomp: Option<Seccomp>,
    /// SELinux configuration
    #[validate(custom = "validation::selinux")]
    pub selinux: Option<selinux::Selinux>,
    /// Capabilities
    #[serde(
        default,
        skip_serializing_if = "HashSet::is_empty",
        deserialize_with = "sets_duplicate_value_is_error::deserialize"
    )]
    pub capabilities: HashSet<capabilities::Capability>,
    /// String containing group names to give to new container
    #[serde(
        default,
        skip_serializing_if = "HashSet::is_empty",
        deserialize_with = "sets_duplicate_value_is_error::deserialize"
    )]
    #[validate(custom = "validation::suppl_groups")]
    pub suppl_groups: HashSet<NonNulString>,
    /// Resource limits
    #[serde(
        default,
        skip_serializing_if = "HashMap::is_empty",
        deserialize_with = "maps_duplicate_key_is_error::deserialize"
    )]
    pub rlimits: HashMap<rlimit::RLimitResource, rlimit::RLimitValue>,
    /// IO configuration
    #[serde(default)]
    pub io: Option<io::Io>,
    /// Optional custom data. The runtime doesn't use this.
    pub custom: Option<Value>,
}

impl Manifest {
    /// Container that is specified in the manifest
    pub fn container(&self) -> Container {
        Container::new(self.name.clone(), self.version.clone())
    }

    /// Read a manifest from `reader`
    pub fn from_reader<R: std::io::Read>(reader: R) -> Result<Self, Error> {
        let manifest: Self = serde_yaml::from_reader(reader)?;
        manifest.validate().map_err(Error::Validation)?;
        Ok(manifest)
    }

    /// Write the manifest to `writer`
    pub fn to_writer<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
        serde_yaml::to_writer(writer, self)?;
        Ok(())
    }
}

impl FromStr for Manifest {
    type Err = Error;

    fn from_str(s: &str) -> Result<Manifest, Self::Err> {
        let manifest: Self = serde_yaml::from_str(s)?;
        manifest.validate().map_err(Error::Validation)?;
        Ok(manifest)
    }
}

impl ToString for Manifest {
    #[allow(clippy::unwrap_used)]
    fn to_string(&self) -> String {
        // A `Manifest` is convertible to `String` as long as its implementation of `Serialize` does
        // not return an error. This should never happen for the types that we use in `Manifest` so
        // we can safely use .unwrap() here.
        serde_yaml::to_string(self).expect("failed to serialize manifest")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::mount::*;
    use crate::{common::version::VersionReq, npk::manifest::*, seccomp::SyscallRule};
    use anyhow::Result;
    use std::{
        convert::{TryFrom, TryInto},
        iter::FromIterator,
    };

    fn nn(s: &str) -> NonNulString {
        unsafe { NonNulString::from_str_unchecked(s) }
    }

    #[test]
    fn parse() -> Result<()> {
        let manifest = "
name: hello
version: 0.0.0
init: /binary
args:
  - one
  - two
env:
  LD_LIBRARY_PATH: /lib
uid: 1000
gid: 1001
suppl_groups:
  - inet
  - log
capabilities:
  - CAP_NET_RAW
  - CAP_MKNOD
  - CAP_SYS_TIME
rlimits:
  nproc:
    soft: 1000
    hard: 1000
mounts:
  /dev:
    type: dev
  /tmp:
    type: tmpfs
    size: 42
  /lib:
    type: bind
    host: /lib
    options: rw
  /data:
    type: persist
  /resource:
    type: resource
    name: bla-blah.foo
    version: '>=1.0.0'
    dir: /bin/foo
    options: noexec
autostart: critical
seccomp:
  allow:
    fork: any
    waitpid: any
cgroups:
    memory:
      memory_hard_limit: 1000000
      memory_soft_limit: 1000000
      swappiness: 0
      attrs: {}
    cpu:
      cpus: 0,1
      shares: 1024
      attrs: {}
";

        let manifest = Manifest::from_str(manifest)?;

        assert_eq!(manifest.init, NonNulString::try_from("/binary").ok());
        assert_eq!(manifest.name.to_string(), "hello");
        assert_eq!(manifest.args.len(), 2);
        assert_eq!(manifest.args[0].to_string(), "one");
        assert_eq!(manifest.args[1].to_string(), "two");

        assert_eq!(manifest.autostart, Some(autostart::Autostart::Critical));
        assert_eq!(
            manifest.env.get(&"LD_LIBRARY_PATH".try_into()?),
            Some("/lib".try_into()?).as_ref()
        );
        assert_eq!(manifest.uid, 1000);
        assert_eq!(manifest.gid, 1001);
        let mut mounts = HashMap::new();
        mounts.insert(
            nn("/lib"),
            Mount::Bind(Bind {
                host: nn("/lib"),
                options: [MountOption::Rw].iter().cloned().collect(),
            }),
        );
        mounts.insert(nn("/data"), Mount::Persist);
        mounts.insert(
            nn("/resource"),
            Mount::Resource(Resource {
                name: "bla-blah.foo".try_into()?,
                version: VersionReq::parse(">=1.0.0")?,
                dir: unsafe { NonNulString::from_str_unchecked("/bin/foo") },
                options: [MountOption::NoExec].iter().cloned().collect(),
            }),
        );
        mounts.insert(nn("/tmp"), Mount::Tmpfs(Tmpfs { size: 42 }));
        mounts.insert(nn("/dev"), Mount::Dev);
        assert_eq!(manifest.mounts, mounts);

        let mut syscalls: HashMap<NonNulString, SyscallRule> = HashMap::new();
        syscalls.insert(
            NonNulString::try_from("fork".to_string())?,
            SyscallRule::Any,
        );
        syscalls.insert(
            NonNulString::try_from("waitpid".to_string())?,
            SyscallRule::Any,
        );
        assert_eq!(
            manifest.seccomp,
            Some(Seccomp {
                profile: None,
                allow: Some(syscalls)
            })
        );

        assert_eq!(
            manifest.capabilities,
            HashSet::from_iter(
                vec!(
                    capabilities::Capability::CAP_NET_RAW,
                    capabilities::Capability::CAP_MKNOD,
                    capabilities::Capability::CAP_SYS_TIME,
                )
                .drain(..)
            )
        );
        let suppl_groups = unsafe {
            ["inet", "log"]
                .into_iter()
                .map(|s| NonNulString::from_str_unchecked(s))
                .collect()
        };
        assert_eq!(manifest.suppl_groups, suppl_groups);

        Ok(())
    }

    /// Invalid uid
    #[test]
    fn invalid_uid() -> Result<()> {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 0\ngid: 1001";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Invalid gid
    #[test]
    fn invalid_gid() -> Result<()> {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1\ngid: 0";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Invalid selinux context
    #[test]
    fn invalid_selinux_context() -> Result<()> {
        let manifest =
            "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1\ngid: 1\nselinux_context: fo@o";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Invalid suppl group with nul byte
    #[test]
    fn invalid_suppl_group_nul() -> Result<()> {
        let manifest =
            "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1\ngid: 1\nsuppl_groups: [fo\0o]";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Invalid too long suppl group
    #[test]
    fn invalid_suppl_group_too_long() -> Result<()> {
        let manifest =
            "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1\ngid: 1\nsuppl_groups: [looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong]";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Too many suppl groups
    #[test]
    fn invalid_suppl_group_duplicate() -> Result<()> {
        let manifest =
            "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1\ngid: 1\nsuppl_groups: [foo, foo]";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Two mounts on the same target are invalid
    #[test]
    fn duplicate_mount() -> Result<()> {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /dev:
    type: dev
  /dev:
    type: dev
";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Overlapping mounts are invalid
    #[test]
    fn overlapping_mount() -> Result<()> {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /lib/overlapping:
    type: bind
    host: /lib
  /lib/non_overlapping1:
    type: bind
    host: /lib
  /lib/non_overlapping2:
    type: bind
    host: /lib
  /lib/overlapping/foo:
    type: bind
    host: /lib
";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }

    /// Non-overlapping mounts are invalid
    #[test]
    fn non_overlapping_mount() -> Result<()> {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /other_lib1:
    type: bind
    host: /lib
  /lib/non_overlapping1:
    type: bind
    host: /lib
  /other_lib2:
    type: bind
    host: /lib
  /lib/non_overlapping2:
    type: bind
    host: /lib
";
        assert!(Manifest::from_str(manifest).is_ok());
        Ok(())
    }

    /// Resource mount with realtive dir
    #[test]
    #[should_panic]
    fn resource_relative_dir() {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /resource:
    type: resource
    name: bla-blah.foo
    version: '>=1.0.0'
    dir: bin/foo
";
        Manifest::from_str(manifest).unwrap();
    }

    /// Resource mount with absolute dir
    #[test]
    fn resource_absolute() {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /resource:
    type: resource
    name: bla-blah.foo
    version: '>=1.0.0'
    dir: /bin/foo
";
        Manifest::from_str(manifest).unwrap();
    }

    #[test]
    fn tmpfs() {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /a:
    type: tmpfs
    size: 100
  /b:
    type: tmpfs
    size: 100kB
  /c:
    type: tmpfs
    size: 100MB
  /d:
    type: tmpfs
    size: 100GB
";
        let mountpoint = |s| -> NonNulString { unsafe { NonNulString::from_str_unchecked(s) } };
        let manifest = Manifest::from_str(manifest).unwrap();
        assert_eq!(
            manifest.mounts.get(&mountpoint("/a")),
            Some(&Mount::Tmpfs(Tmpfs { size: 100 }))
        );
        assert_eq!(
            manifest.mounts.get(&mountpoint("/b")),
            Some(&Mount::Tmpfs(Tmpfs { size: 100000 }))
        );
        assert_eq!(
            manifest.mounts.get(&mountpoint("/c")),
            Some(&Mount::Tmpfs(Tmpfs { size: 100000000 }))
        );
        assert_eq!(
            manifest.mounts.get(&mountpoint("/d")),
            Some(&Mount::Tmpfs(Tmpfs { size: 100000000000 }))
        );

        // Test a invalid tmpfs size string
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\n uid: 1000\ngid: 1001
mounts:
  /tmp:
    type: tmpfs
    size: 100MB
";
        assert!(Manifest::from_str(manifest).is_err());
    }

    #[test]
    fn dev_minimal() {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001\nmounts:\n  /dev:\n    type: dev";
        assert!(Manifest::from_str(manifest).is_ok());
    }

    #[test]
    fn mount_resource() {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001
mounts:
  /foo:
    type: resource
    name: foo-bar.qwerty12
    version: '>=0.0.1'
    dir: /
    options: rw,noexec,nosuid
";
        Manifest::from_str(manifest).unwrap();
    }

    #[test]
    fn roundtrip() -> Result<()> {
        let m = "
name: hello
version: 0.0.0
init: /binary
uid: 1000
gid: 1001
console:
  permissions: full
args:
  - one
  - two
env:
  LD_LIBRARY_PATH: /lib
mounts:
  /dev:
    type: dev
  /lib:
    type: bind
    host: /lib
    options: rw,nosuid,nodev,noexec
  /no_option:
    type: bind
    host: /foo
  /data:
    type: persist
  /resource:
    type: resource
    name: bla-bar.blah1234
    version: '>=1.0.0'
    dir: /bin/foo
    options: rw,nosuid,nodev,noexec
  /tmp:
    type: tmpfs
    size: 42
autostart: relaxed
rlimits:
  nproc:
    soft: 100
    hard: 1000
seccomp:
  allow:
    fork: any
    waitpid: any
capabilities:
  - CAP_NET_ADMIN
io:
  stdout: pipe
  stderr: pipe
cgroups:
    memory:
      memory_hard_limit: 1000000
      memory_soft_limit: 1000000
      swappiness: 0
      attrs: {}
    cpu:
      cpus: 0,1
      shares: 1024
      attrs: {}
custom:
    blah: foo
    foo: 234
    test:
      - one
      - two
      - three
";

        let manifest = serde_yaml::from_str::<Manifest>(m)?;
        let deserialized = serde_yaml::from_str::<Manifest>(&serde_yaml::to_string(&manifest)?)?;

        assert_eq!(manifest, deserialized);
        Ok(())
    }

    /// Check reserved env keys
    #[test]
    fn env() -> Result<()> {
        let manifest = "name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001\n
env:
  LD_LIBRARY_PATH: /lib
  PATH: /bin";

        assert!(Manifest::from_str(manifest).is_ok());

        let manifest = r"name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001\n
env:
  NORTHSTAR_CONSOLE: foo";
        assert!(Manifest::from_str(manifest).is_err());

        let manifest = r"name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001\n
env:
  NORTHSTAR_NAME: foo";
        assert!(Manifest::from_str(manifest).is_err());

        let manifest = r"name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001\n
env:
  NORTHSTAR_CONTAINER: foo";
        assert!(Manifest::from_str(manifest).is_err());

        let manifest = r"name: hello\nversion: 0.0.0\ninit: /binary\nuid: 1000\ngid: 1001\n
env:
  NORTHSTAR_VERSION: foo";
        assert!(Manifest::from_str(manifest).is_err());
        Ok(())
    }
}
