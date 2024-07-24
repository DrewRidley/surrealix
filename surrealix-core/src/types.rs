use chrono::{DateTime as ChronoDateTime, TimeZone, Utc};
use std::fmt;
use std::str::FromStr;
use std::time::Duration as StdDuration;
use std::{
    collections::HashMap,
    num::{NonZeroU16, NonZeroU64},
};
use surrealdb::sql::{Kind, Permissions};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTime(ChronoDateTime<Utc>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duration(StdDuration);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLink(String);

impl DateTime {
    pub fn now() -> Self {
        DateTime(Utc::now())
    }
}

impl FromStr for DateTime {
    type Err = chrono::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DateTime(s.parse()?))
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl FromStr for Duration {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut total = StdDuration::new(0, 0);
        let mut current_number = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c.is_digit(10) {
                current_number.push(c);
            } else {
                let value = current_number.parse::<u64>().unwrap_or(0);
                current_number.clear();

                match c {
                    'n' => total += StdDuration::from_nanos(value),
                    'u' => total += StdDuration::from_micros(value),
                    'm' if chars.peek() == Some(&'s') => {
                        chars.next();
                        total += StdDuration::from_millis(value);
                    }
                    's' => total += StdDuration::from_secs(value),
                    'm' => total += StdDuration::from_secs(value * 60),
                    'h' => total += StdDuration::from_secs(value * 3600),
                    'd' => total += StdDuration::from_secs(value * 86400),
                    'w' => total += StdDuration::from_secs(value * 604800),
                    'y' => total += StdDuration::from_secs(value * 31536000),
                    _ => return Err("Invalid duration format".into()),
                }
            }
        }

        Ok(Duration(total))
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        let nanos = self.0.subsec_nanos();

        if secs == 0 && nanos == 0 {
            return write!(f, "0s");
        }

        let mut result = String::new();
        if secs >= 31536000 {
            let years = secs / 31536000;
            result.push_str(&format!("{}y", years));
        }
        if secs >= 604800 {
            let weeks = (secs % 31536000) / 604800;
            if weeks > 0 {
                result.push_str(&format!("{}w", weeks));
            }
        }
        if secs >= 86400 {
            let days = (secs % 604800) / 86400;
            if days > 0 {
                result.push_str(&format!("{}d", days));
            }
        }
        if secs >= 3600 {
            let hours = (secs % 86400) / 3600;
            if hours > 0 {
                result.push_str(&format!("{}h", hours));
            }
        }
        if secs >= 60 {
            let minutes = (secs % 3600) / 60;
            if minutes > 0 {
                result.push_str(&format!("{}m", minutes));
            }
        }
        let seconds = secs % 60;
        if seconds > 0 {
            result.push_str(&format!("{}s", seconds));
        }
        if nanos > 0 {
            result.push_str(&format!("{}ns", nanos));
        }

        write!(f, "{}", result)
    }
}

impl RecordLink {
    pub fn new(id: impl Into<String>) -> Self {
        RecordLink(id.into())
    }

    pub fn id(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for DateTime {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }

    impl<'de> Deserialize<'de> for DateTime {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            s.parse().map_err(serde::de::Error::custom)
        }
    }

    impl Serialize for Duration {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }

    impl<'de> Deserialize<'de> for Duration {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            s.parse().map_err(serde::de::Error::custom)
        }
    }

    impl Serialize for RecordLink {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0)
        }
    }

    impl<'de> Deserialize<'de> for RecordLink {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            Ok(RecordLink(s))
        }
    }
}

#[cfg(feature = "miniserde")]
mod miniserde_impls {
    use super::*;
    use miniserde::de::Visitor;
    use miniserde::make_place;
    use miniserde::{json, Deserialize, Serialize};

    make_place!(Place);

    impl Visitor for Place<DateTime> {
        fn string(&mut self, s: &str) -> miniserde::Result<()> {
            match s.parse() {
                Ok(datetime) => {
                    self.out = Some(DateTime(datetime));
                    Ok(())
                }
                Err(_) => Err(miniserde::Error),
            }
        }
    }

    impl Visitor for Place<Duration> {
        fn string(&mut self, s: &str) -> miniserde::Result<()> {
            match s.parse() {
                Ok(duration) => {
                    self.out = Some(duration);
                    Ok(())
                }
                Err(_) => Err(miniserde::Error),
            }
        }
    }

    impl Visitor for Place<RecordLink> {
        fn string(&mut self, s: &str) -> miniserde::Result<()> {
            self.out = Some(RecordLink(s.to_string()));
            Ok(())
        }
    }

    impl Deserialize for DateTime {
        fn begin(out: &mut Option<Self>) -> &mut dyn Visitor {
            Place::new(out)
        }
    }

    impl Deserialize for Duration {
        fn begin(out: &mut Option<Self>) -> &mut dyn Visitor {
            Place::new(out)
        }
    }

    impl Deserialize for RecordLink {
        fn begin(out: &mut Option<Self>) -> &mut dyn Visitor {
            Place::new(out)
        }
    }

    impl Serialize for DateTime {
        fn begin(&self) -> miniserde::ser::Fragment {
            let s = self.to_string();
            miniserde::ser::Fragment::Str(s.into())
        }
    }

    impl Serialize for Duration {
        fn begin(&self) -> miniserde::ser::Fragment {
            let s = self.to_string();
            miniserde::ser::Fragment::Str(s.into())
        }
    }

    impl Serialize for RecordLink {
        fn begin(&self) -> miniserde::ser::Fragment {
            miniserde::ser::Fragment::Str(self.0.clone().into())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedQuery {
    pub query_type: QueryType,
    pub perms: Permissions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryType {
    Scalar(Kind),
    Object(HashMap<String, TypedQuery>),
    Array(Option<Box<TypedQuery>>, Option<NonZeroU64>),
    Record(String),
    Option(Box<TypedQuery>),
}
