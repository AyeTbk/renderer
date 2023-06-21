#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    seconds: u32,
    millis: u32,
}

impl Timestamp {
    pub fn now() -> Self {
        Self::from(std::time::SystemTime::now())
    }

    pub fn seconds_since(&self) -> f64 {
        Self::now().as_seconds() - self.as_seconds()
    }

    pub(crate) fn as_seconds(&self) -> f64 {
        self.seconds as f64 + (self.millis as f64 / 1_000.0)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl From<std::time::SystemTime> for Timestamp {
    fn from(value: std::time::SystemTime) -> Self {
        let dur = value.duration_since(std::time::UNIX_EPOCH).unwrap();
        Self {
            seconds: dur.as_secs() as u32, // Losing the 32 high bits is fine
            millis: dur.subsec_millis(),
        }
    }
}
