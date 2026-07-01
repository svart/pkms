use serde::Serialize;
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct NoteId(String);

impl NoteId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_uuid_format(&self) -> bool {
        uuid::Uuid::parse_str(&self.0).is_ok()
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NoteId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for NoteId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for NoteId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<String> for NoteId {
    fn borrow(&self) -> &String {
        &self.0
    }
}

impl From<String> for NoteId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for NoteId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<NoteId> for String {
    fn from(value: NoteId) -> Self {
        value.0
    }
}

impl PartialEq<&str> for NoteId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for NoteId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<String> for NoteId {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct LinkTarget(String);

impl LinkTarget {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LinkTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for LinkTarget {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for LinkTarget {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for LinkTarget {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<String> for LinkTarget {
    fn borrow(&self) -> &String {
        &self.0
    }
}

impl From<String> for LinkTarget {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for LinkTarget {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<LinkTarget> for String {
    fn from(value: LinkTarget) -> Self {
        value.0
    }
}

impl PartialEq<&str> for LinkTarget {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for LinkTarget {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<String> for LinkTarget {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}
