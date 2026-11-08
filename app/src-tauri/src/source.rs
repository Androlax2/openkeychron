//! Where the commands find devices: the connected keyboards, or, in mock mode, the fixtures in
//! the directory `OPENKEYCHRON_FIXTURE` names.
//!
//! In mock mode each `.toml` fixture in that directory is one device. Every command on it
//! replays the fixture from its start through a `ReplayTransport`, the way the CLI's
//! `--fixture` does, so the UI can read a device as often as it likes. A write is checked
//! against the fixture in memory and goes nowhere else: not to a keyboard, not to the file,
//! and not into the next command.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openkeychron_hid::{
    Fixture, FixtureError, HidInterface, HidTransport, HidapiTransport, KeychronDevice,
    ReplayTransport, TransportError, keychron_via_interfaces,
};

use crate::device;
use crate::error::CommandError;

/// The environment variable that turns mock mode on.
pub const FIXTURE_VARIABLE: &str = "OPENKEYCHRON_FIXTURE";

#[derive(Debug)]
pub enum DeviceSource {
    Hardware,
    Fixtures(Vec<FixtureDevice>),
}

#[derive(Debug)]
pub struct FixtureDevice {
    device: KeychronDevice,
    fixture: Fixture,
}

/// Why mock mode could not start. The app refuses to open rather than fall back to the
/// connected keyboards, which a developer could then mistake for fixtures.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error(
        "{FIXTURE_VARIABLE} is set but empty: set it to a directory of fixtures, or unset it to \
         use the connected keyboards"
    )]
    EmptyVariable,

    #[error("cannot read the fixture directory {directory}: {source}")]
    ReadDirectory {
        directory: PathBuf,
        source: std::io::Error,
    },

    #[error("{directory} holds no .toml fixture")]
    NoFixtures { directory: PathBuf },

    #[error("cannot read the fixture {path}: {source}")]
    ReadFixture {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{path}: {source}")]
    Fixture { path: PathBuf, source: FixtureError },

    #[error(
        "{path} was not recorded from a Keychron VIA interface ({vendor_id:04x}:{product_id:04x}, \
         usage page 0x{usage_page:04x}, usage 0x{usage:02x})"
    )]
    NotKeychronVia {
        path: PathBuf,
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
        usage: u16,
    },
}

/// What a command reads and writes through: a keyboard, or a fixture's in-memory replay.
#[derive(Debug)]
pub enum Transport {
    Hardware(HidapiTransport),
    Fixture(ReplayTransport),
}

impl HidTransport for Transport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        match self {
            Self::Hardware(transport) => transport.write(report),
            Self::Fixture(transport) => transport.write(report),
        }
    }

    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        match self {
            Self::Hardware(transport) => transport.read(timeout),
            Self::Fixture(transport) => transport.read(timeout),
        }
    }
}

impl DeviceSource {
    /// Mock mode when `value` (the variable's value) is set, the connected keyboards otherwise.
    pub fn from_variable(value: Option<OsString>) -> Result<Self, SourceError> {
        match value {
            None => Ok(Self::Hardware),
            Some(directory) if directory.is_empty() => Err(SourceError::EmptyVariable),
            Some(directory) => Self::fixtures(Path::new(&directory)),
        }
    }

    /// One device per `.toml` file in `directory`, in file name order. Other files are
    /// skipped, so a directory can also hold backups and notes.
    pub fn fixtures(directory: &Path) -> Result<Self, SourceError> {
        let read_error = |source| SourceError::ReadDirectory {
            directory: directory.to_owned(),
            source,
        };
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(directory).map_err(read_error)? {
            let path = entry.map_err(read_error)?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                paths.push(path);
            }
        }
        if paths.is_empty() {
            return Err(SourceError::NoFixtures {
                directory: directory.to_owned(),
            });
        }
        paths.sort();
        let devices = paths
            .into_iter()
            .map(|path| FixtureDevice::load(&path))
            .collect::<Result<_, _>>()?;
        Ok(Self::Fixtures(devices))
    }

    pub fn list(&self) -> Result<Vec<KeychronDevice>, CommandError> {
        match self {
            Self::Hardware => Ok(openkeychron_hid::list_keychron_devices()?),
            Self::Fixtures(fixture_devices) => Ok(fixture_devices
                .iter()
                .map(|fixture_device| fixture_device.device.clone())
                .collect()),
        }
    }

    /// The listed device at `path`, opened. A path that is not listed is refused.
    pub fn open(&self, path: &str) -> Result<(KeychronDevice, Transport), CommandError> {
        match self {
            Self::Hardware => {
                let device = device::find(openkeychron_hid::list_keychron_devices()?, path)?;
                let transport = openkeychron_hid::open(&device)?;
                Ok((device, Transport::Hardware(transport)))
            }
            Self::Fixtures(fixture_devices) => {
                let Some(fixture_device) = fixture_devices
                    .iter()
                    .find(|fixture_device| fixture_device.device.path == path)
                else {
                    return Err(device::not_connected(&self.list()?, path));
                };
                let replay = ReplayTransport::new(fixture_device.fixture.clone());
                Ok((fixture_device.device.clone(), Transport::Fixture(replay)))
            }
        }
    }
}

impl FixtureDevice {
    fn load(path: &Path) -> Result<Self, SourceError> {
        // `FixtureError::Read` keeps the I/O reason as its source, which a message alone drops.
        let fixture = Fixture::load(path).map_err(|error| match error {
            FixtureError::Read { path, source } => SourceError::ReadFixture { path, source },
            other => SourceError::Fixture {
                path: path.to_owned(),
                source: other,
            },
        })?;
        let identity = &fixture.device;
        let interface = HidInterface {
            name: Some(identity.name.clone()),
            vendor_id: identity.vendor_id,
            product_id: identity.product_id,
            usage_page: identity.usage_page,
            usage: identity.usage,
            path: format!("fixture:{}", path.display()),
        };
        let device = keychron_via_interfaces([interface]).pop().ok_or_else(|| {
            SourceError::NotKeychronVia {
                path: path.to_owned(),
                vendor_id: identity.vendor_id,
                product_id: identity.product_id,
                usage_page: identity.usage_page,
                usage: identity.usage,
            }
        })?;
        Ok(Self { device, fixture })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use openkeychron_hid::{Event, HidTransport, TransportError};

    use super::{DeviceSource, Transport};
    use crate::keymap;
    use crate::test_support::{load_fixture, synthetic_path};

    const TIMEOUT: Duration = Duration::from_millis(100);

    /// A fresh directory under the system temp directory, removed when dropped.
    struct FixtureDirectory(PathBuf);

    impl FixtureDirectory {
        fn new(test: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "openkeychron-app-mock-{test}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        /// Copies a synthetic fixture in under `name`.
        fn with_synthetic(self, name: &str, synthetic: &str) -> Self {
            std::fs::copy(synthetic_path(synthetic), self.0.join(name)).unwrap();
            self
        }

        fn with_file(self, name: &str, contents: &str) -> Self {
            std::fs::write(self.0.join(name), contents).unwrap();
            self
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn device_path(&self, name: &str) -> String {
            format!("fixture:{}", self.0.join(name).display())
        }
    }

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_transport(transport: Transport) -> openkeychron_hid::ReplayTransport {
        match transport {
            Transport::Fixture(replay) => replay,
            Transport::Hardware(_) => panic!("mock mode opened a keyboard"),
        }
    }

    #[test]
    fn without_the_variable_the_commands_use_the_connected_keyboards() {
        assert!(matches!(
            DeviceSource::from_variable(None),
            Ok(DeviceSource::Hardware)
        ));
    }

    #[test]
    fn the_variable_turns_on_mock_mode_with_its_directory() {
        let directory =
            FixtureDirectory::new("variable").with_synthetic("keymap.toml", "cli-keymap.toml");
        let source =
            DeviceSource::from_variable(Some(directory.path().as_os_str().to_owned())).unwrap();
        assert_eq!(
            source
                .list()
                .unwrap()
                .into_iter()
                .map(|device| device.path)
                .collect::<Vec<_>>(),
            [directory.device_path("keymap.toml")]
        );
    }

    #[test]
    fn refuses_an_empty_variable() {
        let error = DeviceSource::from_variable(Some(OsString::new())).unwrap_err();
        assert_eq!(
            error.to_string(),
            "OPENKEYCHRON_FIXTURE is set but empty: set it to a directory of fixtures, or unset \
             it to use the connected keyboards"
        );
    }

    #[test]
    fn lists_one_device_per_toml_fixture_in_file_name_order() {
        let directory = FixtureDirectory::new("list")
            .with_synthetic("b-settings.toml", "cli-settings-get.toml")
            .with_synthetic("a-keymap.toml", "cli-keymap.toml")
            .with_synthetic("backup.json", "keymap-backup.json");
        let devices = DeviceSource::fixtures(directory.path())
            .unwrap()
            .list()
            .unwrap();
        assert_eq!(
            devices
                .iter()
                .map(|device| (device.name.as_deref(), device.path.clone()))
                .collect::<Vec<_>>(),
            [
                (
                    Some("synthetic V5 Max ISO keymap"),
                    directory.device_path("a-keymap.toml")
                ),
                (
                    Some("Keychron V5 Max"),
                    directory.device_path("b-settings.toml")
                ),
            ]
        );
    }

    #[test]
    fn a_fixture_recorded_through_the_receiver_lists_as_a_receiver() {
        let directory =
            FixtureDirectory::new("receiver").with_synthetic("link.toml", "cli-info.toml");
        let devices = DeviceSource::fixtures(directory.path())
            .unwrap()
            .list()
            .unwrap();
        assert!(devices[0].is_receiver);
    }

    #[test]
    fn refuses_a_directory_without_a_toml_fixture() {
        let directory =
            FixtureDirectory::new("empty").with_synthetic("backup.json", "keymap-backup.json");
        let error = DeviceSource::fixtures(directory.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("{} holds no .toml fixture", directory.path().display())
        );
    }

    #[test]
    fn refuses_a_directory_that_does_not_exist() {
        let directory = FixtureDirectory::new("missing");
        let missing = directory.path().join("nope");
        let error = DeviceSource::fixtures(&missing).unwrap_err();
        assert!(
            error.to_string().starts_with(&format!(
                "cannot read the fixture directory {}: ",
                missing.display()
            )),
            "{error}"
        );
    }

    #[test]
    fn refuses_a_fixture_that_does_not_parse_and_names_its_file() {
        let directory =
            FixtureDirectory::new("invalid").with_file("broken.toml", "format_version = 2\n");
        let error = DeviceSource::fixtures(directory.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}: fixture format_version 2 is not supported (this build reads 1)",
                directory.path().join("broken.toml").display()
            )
        );
    }

    #[test]
    fn refuses_a_fixture_it_cannot_read_and_says_why() {
        let directory = FixtureDirectory::new("unreadable");
        let not_a_file = directory.path().join("old.toml");
        std::fs::create_dir(&not_a_file).unwrap();
        let error = DeviceSource::fixtures(directory.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "cannot read the fixture {}: Is a directory (os error 21)",
                not_a_file.display()
            )
        );
    }

    #[test]
    fn refuses_a_fixture_not_recorded_from_a_keychron_via_interface() {
        let other_vendor = std::fs::read_to_string(synthetic_path("cli-keymap.toml"))
            .unwrap()
            .replace("vendor_id = 13364", "vendor_id = 0x1234");
        let directory = FixtureDirectory::new("vendor").with_file("other.toml", &other_vendor);
        let error = DeviceSource::fixtures(directory.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{} was not recorded from a Keychron VIA interface (1234:0951, usage page \
                 0xff60, usage 0x61)",
                directory.path().join("other.toml").display()
            )
        );
    }

    #[test]
    fn refuses_to_open_a_path_that_is_not_listed() {
        let directory =
            FixtureDirectory::new("unlisted").with_synthetic("keymap.toml", "cli-keymap.toml");
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let error = source.open("/dev/hidraw0").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "/dev/hidraw0 is not a connected Keychron VIA interface; connected: {}",
                directory.device_path("keymap.toml")
            )
        );
    }

    #[test]
    fn a_fixture_device_answers_a_read_command_from_its_fixture() {
        let directory =
            FixtureDirectory::new("read").with_synthetic("keymap.toml", "cli-keymap.toml");
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let (device, transport) = source.open(&directory.device_path("keymap.toml")).unwrap();
        let keymap = keymap::read(transport, &device).unwrap();
        assert_eq!(keymap.layers[0].keys[1].keycode, 0xdead);
    }

    #[test]
    fn every_command_replays_the_fixture_from_its_start() {
        let directory =
            FixtureDirectory::new("again").with_synthetic("keymap.toml", "cli-keymap.toml");
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let path = directory.device_path("keymap.toml");
        for _ in 0..2 {
            let (device, transport) = source.open(&path).unwrap();
            assert_eq!(keymap::read(transport, &device).unwrap().layer_count, 2);
        }
    }

    #[test]
    fn a_recorded_write_is_answered_in_memory_and_leaves_the_fixture_file_unchanged() {
        let directory =
            FixtureDirectory::new("write").with_synthetic("set.toml", "cli-keymap-set.toml");
        let file = directory.path().join("set.toml");
        let before = std::fs::read(&file).unwrap();
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let (_, transport) = source.open(&directory.device_path("set.toml")).unwrap();
        let mut replay = fixture_transport(transport);
        for event in load_fixture("cli-keymap-set.toml").events {
            match event {
                Event::Write { data } => assert_eq!(replay.write(&data), Ok(())),
                Event::Read { data } => assert_eq!(replay.read(TIMEOUT), Ok(data)),
                Event::ReadTimeout => {
                    assert_eq!(replay.read(TIMEOUT), Err(TransportError::Timeout));
                }
            }
        }
        assert_eq!(replay.finish(), Ok(()));
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }

    #[test]
    fn refuses_a_write_the_fixture_did_not_record() {
        let directory =
            FixtureDirectory::new("unrecorded").with_synthetic("set.toml", "cli-keymap-set.toml");
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let (_, transport) = source.open(&directory.device_path("set.toml")).unwrap();
        let mut replay = fixture_transport(transport);
        assert!(matches!(
            replay.write(&[0x05, 0x00, 0x02, 0x01, 0x00, 0x04]),
            Err(TransportError::UnexpectedWrite { position: 0, .. })
        ));
    }

    #[test]
    fn a_write_does_not_carry_over_into_the_next_command() {
        let directory =
            FixtureDirectory::new("carry").with_synthetic("set.toml", "cli-keymap-set.toml");
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let path = directory.device_path("set.toml");
        let first_write = match &load_fixture("cli-keymap-set.toml").events[0] {
            Event::Write { data } => data.clone(),
            other => panic!("the fixture starts with {other:?}, not a write"),
        };
        let (_, transport) = source.open(&path).unwrap();
        fixture_transport(transport).write(&first_write).unwrap();
        let (_, transport) = source.open(&path).unwrap();
        assert_eq!(fixture_transport(transport).write(&first_write), Ok(()));
    }
}
