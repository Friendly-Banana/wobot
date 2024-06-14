use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Deref;

use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug)]
pub(crate) struct HashableRegex(Regex);

impl Display for HashableRegex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

impl Deref for HashableRegex {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash for HashableRegex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl PartialEq for HashableRegex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for HashableRegex {}

impl<'de> Deserialize<'de> for HashableRegex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Regex::new(&format!("\\b{}\\b", s))
            .map(HashableRegex)
            .map_err(Error::custom)
    }
}
