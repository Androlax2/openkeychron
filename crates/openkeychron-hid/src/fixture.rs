use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The only fixture format this build reads and writes. Bump it on any schema change.
pub const FIXTURE_FORMAT_VERSION: u32 = 1;

/// One recorded HID session: which interface it came from and every report, in order.
///
/// Unknown keys are rejected, so a serial number or another identifier cannot slip into a
/// committed fixture: the schema has no place for one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub format_version: u32,
    pub device: DeviceIdentity,
    #[serde(default, rename = "event")]
    pub events: Vec<Event>,
}

/// The model-level identity of the recorded interface, never the unit's serial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceIdentity {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Write {
        #[serde(with = "hex_bytes")]
        data: Vec<u8>,
    },
    Read {
        #[serde(with = "hex_bytes")]
        data: Vec<u8>,
    },
    ReadTimeout,
}

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("cannot read fixture {path}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("cannot write fixture {path}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid fixture: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("cannot encode fixture: {0}")]
    Encode(#[from] toml::ser::Error),

    #[error(
        "fixture format_version {found} is not supported (this build reads {FIXTURE_FORMAT_VERSION})"
    )]
    UnsupportedVersion { found: u32 },
}

impl Fixture {
    #[must_use]
    pub fn new(device: DeviceIdentity, events: Vec<Event>) -> Self {
        Self {
            format_version: FIXTURE_FORMAT_VERSION,
            device,
            events,
        }
    }

    /// Parses a fixture, checking `format_version` before the rest of the schema so an old
    /// or newer file fails with a version error instead of a confusing field error.
    pub fn from_toml(text: &str) -> Result<Self, FixtureError> {
        #[derive(Deserialize)]
        struct VersionProbe {
            format_version: u32,
        }

        let probe: VersionProbe = toml::from_str(text)?;
        if probe.format_version != FIXTURE_FORMAT_VERSION {
            return Err(FixtureError::UnsupportedVersion {
                found: probe.format_version,
            });
        }
        Ok(toml::from_str(text)?)
    }

    pub fn to_toml(&self) -> Result<String, FixtureError> {
        Ok(toml::to_string(self)?)
    }

    pub fn load(path: &Path) -> Result<Self, FixtureError> {
        let text = std::fs::read_to_string(path).map_err(|source| FixtureError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::from_toml(&text)
    }

    pub fn save(&self, path: &Path) -> Result<(), FixtureError> {
        std::fs::write(path, self.to_toml()?).map_err(|source| FixtureError::Write {
            path: path.to_owned(),
            source,
        })
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub(super) fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&crate::hex::encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        crate::hex::decode(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceIdentity, Event, Fixture, FixtureError};

    fn keychron_link() -> DeviceIdentity {
        DeviceIdentity {
            name: "Keychron Link".to_owned(),
            vendor_id: 0x3434,
            product_id: 0xd030,
            usage_page: 0xff60,
            usage: 0x61,
        }
    }

    const VALID: &str = r#"
format_version = 1

[device]
name = "Keychron Link"
vendor_id = 0x3434
product_id = 0xd030
usage_page = 0xff60
usage = 0x61

[[event]]
kind = "write"
data = "01 00"

[[event]]
kind = "read"
data = "01 00 0c"

[[event]]
kind = "read_timeout"
"#;

    #[test]
    fn parses_a_hand_written_fixture() {
        let fixture = Fixture::from_toml(VALID).unwrap();
        assert_eq!(fixture.device, keychron_link());
        assert_eq!(
            fixture.events,
            vec![
                Event::Write {
                    data: vec![0x01, 0x00]
                },
                Event::Read {
                    data: vec![0x01, 0x00, 0x0c]
                },
                Event::ReadTimeout,
            ]
        );
    }

    #[test]
    fn round_trips_through_toml() {
        let fixture = Fixture::new(
            keychron_link(),
            vec![Event::Write { data: vec![0x01] }, Event::ReadTimeout],
        );
        assert_eq!(
            Fixture::from_toml(&fixture.to_toml().unwrap()).unwrap(),
            fixture
        );
    }

    #[test]
    fn rejects_another_format_version() {
        let newer = VALID.replace("format_version = 1", "format_version = 2");
        assert!(matches!(
            Fixture::from_toml(&newer),
            Err(FixtureError::UnsupportedVersion { found: 2 })
        ));
    }

    #[test]
    fn rejects_a_serial_number_on_the_device() {
        let with_serial = VALID.replace("usage = 0x61", "usage = 0x61\nserial_number = \"A1B2C3\"");
        assert!(matches!(
            Fixture::from_toml(&with_serial),
            Err(FixtureError::Parse(_))
        ));
    }

    #[test]
    fn rejects_an_unknown_key_on_an_event() {
        let with_extra = VALID.replace("data = \"01 00\"", "data = \"01 00\"\nnote = \"x\"");
        assert!(matches!(
            Fixture::from_toml(&with_extra),
            Err(FixtureError::Parse(_))
        ));
    }

    #[test]
    fn rejects_bad_hex_in_event_data() {
        let bad_hex = VALID.replace("data = \"01 00\"", "data = \"01 zz\"");
        assert!(matches!(
            Fixture::from_toml(&bad_hex),
            Err(FixtureError::Parse(_))
        ));
    }
}
