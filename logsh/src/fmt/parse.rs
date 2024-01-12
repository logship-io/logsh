use core::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
pub struct OptionalDurationArg {
    duration: Option<std::time::Duration>,
}

impl FromStr for OptionalDurationArg {
    type Err = humantime::DurationError;

    fn from_str(arg: &str) -> Result<Self, Self::Err> {
        if arg.trim().to_lowercase() == "none" {
            return Ok(Self { duration: None });
        }

        let duration = humantime::parse_duration(arg)?;
        log::debug!("Parsed duration seconds: {}", duration.as_secs());
        Ok(Self {
            duration: Some(duration),
        })
    }
}

impl From<OptionalDurationArg> for Option<std::time::Duration> {
    fn from(val: OptionalDurationArg) -> Self {
        val.duration
    }
}

impl AsRef<Option<std::time::Duration>> for OptionalDurationArg {
    fn as_ref(&self) -> &Option<std::time::Duration> {
        &self.duration
    }
}

impl AsMut<Option<std::time::Duration>> for OptionalDurationArg {
    fn as_mut(&mut self) -> &mut Option<std::time::Duration> {
        &mut self.duration
    }
}

impl fmt::Display for OptionalDurationArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.duration {
            Some(duration) => humantime::format_duration(duration).fmt(f),
            None => write!(f, "None"),
        }
    }
}
