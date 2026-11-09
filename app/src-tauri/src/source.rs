//! Where the commands find devices: the connected keyboards, or, in mock mode, the fixtures in
//! the directory `OPENKEYCHRON_FIXTURE` names.
//!
//! In mock mode each `.toml` file in that directory is one device:
//!
//! - A mock keyboard file (`*.mock.toml`) is a stateful fake keyboard, a `MockKeyboard`. It
//!   answers every read command from its state, and a write changes that state, so the next
//!   command reads it back. The state lives in memory until the app quits.
//! - Any other `.toml` file is a fixture. Every command on it replays the fixture from its start
//!   through a `ReplayTransport`, the way the CLI's `--fixture` does, so the UI can read a device
//!   as often as it likes. A write is checked against the fixture in memory and goes nowhere
//!   else: not to a keyboard, not to the file, and not into the next command.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use openkeychron_hid::{
    Fixture, FixtureError, HidTransport, HidapiTransport, KeychronDevice, NotKeychronVia,
    ReplayTransport, TransportError, fixture_device,
};
use openkeychron_mock::{MockError, MockKeyboard, MockTransport};

use crate::device;
use crate::error::CommandError;

/// The environment variable that turns mock mode on.
pub const FIXTURE_VARIABLE: &str = "OPENKEYCHRON_FIXTURE";

/// The file name ending that makes a `.toml` file a mock keyboard rather than a fixture. A mock
/// keyboard file also parses as a fixture without events, so the name decides.
const MOCK_SUFFIX: &str = ".mock.toml";

/// Where Linux lists each hidraw node, the only kind of path the `hidapi` backend gives.
const SYSFS_HIDRAW: &str = "/sys/class/hidraw";

#[derive(Debug)]
pub enum DeviceSource {
    Hardware,
    Fixtures(Vec<FixtureDevice>),
}

#[derive(Debug)]
pub struct FixtureDevice {
    device: KeychronDevice,
    answers: Answers,
}

/// What answers a mock mode device's commands.
#[derive(Debug)]
enum Answers {
    Replay(Fixture),
    Mock(MockKeyboard),
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

    #[error("{path}: {source}")]
    Mock { path: PathBuf, source: MockError },

    #[error(transparent)]
    NotKeychronVia(#[from] NotKeychronVia),
}

/// One plug-in of a device, which a keymap history belongs to. Linux names each HID device
/// anew when it is plugged in (`0003:3434:0951.0007`, from a counter that only goes up), so a
/// keyboard plugged in again on the same path, or another keyboard taking that path, is another
/// connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Connection {
    /// The path `list_devices` gave.
    pub path: String,
    /// The kernel's name for the HID device at `path`. `None` for a mock mode device, which is
    /// never unplugged, so its path is enough.
    pub hid_device: Option<String>,
}

/// What a command reads and writes through: a keyboard, a fixture's in-memory replay, or a mock
/// keyboard.
#[derive(Debug)]
pub enum Transport {
    Hardware(HidapiTransport),
    Fixture(ReplayTransport),
    Mock(MockTransport),
}

impl HidTransport for Transport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        match self {
            Self::Hardware(transport) => transport.write(report),
            Self::Fixture(transport) => transport.write(report),
            Self::Mock(transport) => transport.write(report),
        }
    }

    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        match self {
            Self::Hardware(transport) => transport.read(timeout),
            Self::Fixture(transport) => transport.read(timeout),
            Self::Mock(transport) => transport.read(timeout),
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

    /// One device per `.toml` file in `directory`, in file name order: a mock keyboard for a
    /// `*.mock.toml` file, a fixture for any other. Other files are skipped, so a directory can
    /// also hold backups and notes.
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
                let transport = match &fixture_device.answers {
                    Answers::Replay(fixture) => {
                        Transport::Fixture(ReplayTransport::new(fixture.clone()))
                    }
                    Answers::Mock(keyboard) => Transport::Mock(keyboard.connect()),
                };
                Ok((fixture_device.device.clone(), transport))
            }
        }
    }

    /// `open`, and which plug-in of the device the transport reached. The connection is read
    /// after opening, so it is never older than the transport.
    pub fn open_connection(
        &self,
        path: &str,
    ) -> Result<(KeychronDevice, Transport, Connection), CommandError> {
        let (device, transport) = self.open(path)?;
        let connection = match self {
            Self::Hardware => hidraw_connection(Path::new(SYSFS_HIDRAW), &device.path)?,
            Self::Fixtures(_) => Connection {
                path: device.path.clone(),
                hid_device: None,
            },
        };
        Ok((device, transport, connection))
    }
}

/// The connection of the hidraw node at `path` (`/dev/hidrawN`): the HID device its entry in
/// `sysfs_hidraw` links to.
fn hidraw_connection(sysfs_hidraw: &Path, path: &str) -> Result<Connection, CommandError> {
    let unknown = |reason: String| CommandError::UnknownConnection {
        path: path.to_owned(),
        reason,
    };
    let node = Path::new(path)
        .file_name()
        .ok_or_else(|| unknown("the path names no hidraw node".to_owned()))?;
    let link = sysfs_hidraw.join(node).join("device");
    let target = std::fs::read_link(&link)
        .map_err(|error| unknown(format!("reading {}: {error}", link.display())))?;
    let hid_device = target
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| unknown(format!("{} links to {}", link.display(), target.display())))?;
    Ok(Connection {
        path: path.to_owned(),
        hid_device: Some(hid_device.to_owned()),
    })
}

impl FixtureDevice {
    fn load(path: &Path) -> Result<Self, SourceError> {
        let is_mock = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(MOCK_SUFFIX));
        if is_mock {
            let keyboard = MockKeyboard::load(path).map_err(|source| SourceError::Mock {
                path: path.to_owned(),
                source,
            })?;
            let device = fixture_device(keyboard.identity(), path)?;
            return Ok(Self {
                device,
                answers: Answers::Mock(keyboard),
            });
        }
        // `FixtureError::Read` keeps the I/O reason as its source, which a message alone drops.
        let fixture = Fixture::load(path).map_err(|error| match error {
            FixtureError::Read { path, source } => SourceError::ReadFixture { path, source },
            other => SourceError::Fixture {
                path: path.to_owned(),
                source: other,
            },
        })?;
        let device = fixture_device(&fixture.device, path)?;
        Ok(Self {
            device,
            answers: Answers::Replay(fixture),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use openkeychron_guard::keymap::{self as guarded_keymap, KeyPosition, KeymapBackup};
    use openkeychron_guard::{GuardError, require_write_verified};
    use openkeychron_hid::{Event, HidTransport, TransportError};
    use openkeychron_via::Via;

    use super::{Connection, DeviceSource, Transport, hidraw_connection};
    use crate::info::{self, KeychronFirmware};
    use crate::test_support::{load_fixture, synthetic_path};
    use crate::{keymap, macros, settings};

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
            Transport::Mock(_) => panic!("a fixture opened as a mock keyboard"),
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
            FixtureDirectory::new("read").with_synthetic("keymap.toml", "app-keymap.toml");
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let (device, transport) = source.open(&directory.device_path("keymap.toml")).unwrap();
        let keymap = keymap::read(transport, &device).unwrap();
        assert_eq!(keymap.layers[0].keys[1].keycode, 0xdead);
    }

    #[test]
    fn every_command_replays_the_fixture_from_its_start() {
        let directory =
            FixtureDirectory::new("again").with_synthetic("keymap.toml", "app-keymap.toml");
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

    const MOCK: &str = "v5-max-iso.mock.toml";
    const KEYS_PER_LAYER: usize = 6 * 19;
    /// Row 2, column 1: the key labelled `A` on the FR ISO layout.
    const ROW: u8 = 2;
    const COLUMN: u8 = 1;
    const KC_A: u16 = 0x0004;
    const KC_B: u16 = 0x0005;

    fn mock_directory(test: &str) -> (FixtureDirectory, DeviceSource, String) {
        let directory = FixtureDirectory::new(test).with_synthetic("keyboard.mock.toml", MOCK);
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        let path = directory.device_path("keyboard.mock.toml");
        (directory, source, path)
    }

    fn keycode_at(source: &DeviceSource, path: &str) -> u16 {
        let (device, transport) = source.open(path).unwrap();
        keymap::read(transport, &device).unwrap().layers[0].keys
            [usize::from(ROW) * 19 + usize::from(COLUMN)]
        .keycode
    }

    /// Sets the key at `ROW`, `COLUMN` on layer 0 the way the app's keymap write will: through
    /// the guarded write path, on a newly opened device. Returns the backup, its undo.
    fn set_keycode(source: &DeviceSource, path: &str, keycode: u16) -> KeymapBackup {
        let (device, transport) = source.open(path).unwrap();
        let verified = require_write_verified("keymap writes", &device).unwrap();
        guarded_keymap::set_keycode::<_, GuardError>(
            &mut Via::new(transport),
            &verified,
            0,
            KeyPosition::new(ROW, COLUMN).unwrap(),
            keycode,
            |_| Ok(()),
        )
        .unwrap()
    }

    #[test]
    fn a_mock_keyboard_file_lists_as_one_cabled_device_under_its_name() {
        let (directory, source, _) = mock_directory("mock-list");
        let devices = source.list().unwrap();
        assert_eq!(
            devices
                .iter()
                .map(|device| (
                    device.name.as_deref(),
                    device.path.clone(),
                    device.is_receiver
                ))
                .collect::<Vec<_>>(),
            [(
                Some("mock V5 Max ISO"),
                directory.device_path("keyboard.mock.toml"),
                false
            )]
        );
    }

    #[test]
    fn one_mock_device_answers_every_read_command() {
        let (_directory, source, path) = mock_directory("mock-reads");
        let (device, transport) = source.open(&path).unwrap();
        let info = info::read(transport, &device).unwrap();
        let (device, transport) = source.open(&path).unwrap();
        let keymap = keymap::read(transport, &device).unwrap();
        let (device, transport) = source.open(&path).unwrap();
        let macros = macros::read(transport, &device).unwrap();
        let (_, transport) = source.open(&path).unwrap();
        let settings = settings::read(transport).unwrap();
        assert_eq!(
            (
                info.keychron_firmware,
                keymap.layer_count,
                keymap.layers[3].keys.len(),
                macros.macro_count,
                settings.debounce.time_ms,
            ),
            (
                KeychronFirmware::Reported {
                    version: "v0.0.0".to_owned(),
                    build_date: "mock".to_owned(),
                },
                4,
                KEYS_PER_LAYER,
                16,
                50
            )
        );
    }

    #[test]
    fn a_guarded_keymap_write_on_a_mock_device_shows_in_the_next_read_keymap() {
        let (_directory, source, path) = mock_directory("mock-write");
        set_keycode(&source, &path, KC_A);
        assert_eq!(keycode_at(&source, &path), KC_A);
    }

    #[test]
    fn restoring_the_backup_of_a_mock_keymap_write_shows_the_old_key_again() {
        let (_directory, source, path) = mock_directory("mock-undo");
        // A non-zero key, so a restore that wrote the zero seed back would not pass.
        set_keycode(&source, &path, KC_B);
        let backup = set_keycode(&source, &path, KC_A);
        let (device, transport) = source.open(&path).unwrap();
        let verified = require_write_verified("keymap writes", &device).unwrap();
        guarded_keymap::restore::<_, GuardError>(
            &mut Via::new(transport),
            &verified,
            &backup,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(keycode_at(&source, &path), KC_B);
    }

    #[test]
    fn each_mock_device_keeps_its_own_state() {
        let directory = FixtureDirectory::new("mock-two")
            .with_synthetic("a.mock.toml", MOCK)
            .with_synthetic("b.mock.toml", MOCK);
        let source = DeviceSource::fixtures(directory.path()).unwrap();
        set_keycode(&source, &directory.device_path("a.mock.toml"), KC_A);
        assert_eq!(
            keycode_at(&source, &directory.device_path("b.mock.toml")),
            0
        );
    }

    #[test]
    fn refuses_an_invalid_mock_keyboard_file_and_names_it() {
        let directory = FixtureDirectory::new("mock-invalid")
            .with_file("broken.mock.toml", "format_version = 1\n");
        let error = DeviceSource::fixtures(directory.path()).unwrap_err();
        assert!(
            error.to_string().starts_with(&format!(
                "{}: invalid mock keyboard: ",
                directory.path().join("broken.mock.toml").display()
            )),
            "{error}"
        );
    }

    #[test]
    fn refuses_a_mock_keyboard_for_a_model_without_a_built_in_layout_and_names_it() {
        let other_model = std::fs::read_to_string(synthetic_path(MOCK))
            .unwrap()
            .replace("product_id = 2385", "product_id = 2384");
        let directory =
            FixtureDirectory::new("mock-model").with_file("other.mock.toml", &other_model);
        let error = DeviceSource::fixtures(directory.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}: this build has no keyboard layout for 3434:0950, so a mock keyboard cannot \
                 size its keymap",
                directory.path().join("other.mock.toml").display()
            )
        );
    }

    /// A stand-in for `/sys/class/hidraw`, where each node links to the HID device behind it.
    struct Sysfs(FixtureDirectory);

    impl Sysfs {
        fn new(test: &str) -> Self {
            Self(FixtureDirectory::new(test))
        }

        /// Plugs `hid_device` in at `node`, as the kernel would: its sysfs entry links to it.
        fn plug_in(&self, node: &str, hid_device: &str) {
            let entry = self.0.path().join(node);
            std::fs::create_dir_all(&entry).unwrap();
            let link = entry.join("device");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(format!("../../../{hid_device}"), link).unwrap();
        }

        fn connection(&self, path: &str) -> Result<Connection, crate::error::CommandError> {
            hidraw_connection(self.0.path(), path)
        }
    }

    #[test]
    fn a_hidraw_device_is_connected_as_the_hid_device_its_sysfs_node_links_to() {
        let sysfs = Sysfs::new("sysfs-link");
        sysfs.plug_in("hidraw5", "0003:3434:0951.0007");
        assert_eq!(
            sysfs.connection("/dev/hidraw5").unwrap(),
            Connection {
                path: "/dev/hidraw5".to_owned(),
                hid_device: Some("0003:3434:0951.0007".to_owned()),
            }
        );
    }

    #[test]
    fn a_device_plugged_in_again_on_the_same_path_is_another_connection() {
        let sysfs = Sysfs::new("sysfs-replug");
        sysfs.plug_in("hidraw5", "0003:3434:0951.0007");
        let first = sysfs.connection("/dev/hidraw5").unwrap();
        sysfs.plug_in("hidraw5", "0003:3434:0951.0008");
        assert_ne!(sysfs.connection("/dev/hidraw5").unwrap(), first);
    }

    #[test]
    fn refuses_a_hidraw_node_without_a_sysfs_entry_and_names_it() {
        let sysfs = Sysfs::new("sysfs-missing");
        let error = sysfs.connection("/dev/hidraw5").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "cannot tell which keyboard is at /dev/hidraw5, so keymap commands on it are \
                 refused: reading {}: No such file or directory (os error 2); plug the keyboard \
                 in again",
                sysfs.0.path().join("hidraw5/device").display()
            )
        );
    }

    #[test]
    fn a_mock_device_stays_one_connection_across_commands() {
        let (_directory, source, path) = mock_directory("mock-connection");
        let (_, _, first) = source.open_connection(&path).unwrap();
        let (_, _, again) = source.open_connection(&path).unwrap();
        assert_eq!(first, again);
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
