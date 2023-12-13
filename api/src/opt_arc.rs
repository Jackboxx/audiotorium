use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug)]
pub struct OptionArcStr {
    inner: Option<Arc<str>>,
}

impl Clone for OptionArcStr {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.as_ref().map(Arc::clone),
        }
    }
}

impl From<OptionArcStr> for Option<Arc<str>> {
    fn from(value: OptionArcStr) -> Self {
        value.inner
    }
}

impl From<Option<Arc<str>>> for OptionArcStr {
    fn from(value: Option<Arc<str>>) -> Self {
        Self { inner: value }
    }
}

impl From<Option<String>> for OptionArcStr {
    fn from(value: Option<String>) -> Self {
        Self {
            inner: value.map(|str| str.into()),
        }
    }
}

impl Serialize for OptionArcStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.inner {
            Some(str) => serializer.serialize_str(str),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for OptionArcStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = Option::<Arc<str>>::deserialize(deserializer)?;
        Ok(Self { inner: data })
    }
}

impl TS for OptionArcStr {
    fn name() -> String {
        "string | null".to_string()
    }

    fn dependencies() -> Vec<ts_rs::Dependency>
    where
        Self: 'static,
    {
        vec![]
    }

    fn transparent() -> bool {
        false
    }
}

impl OptionArcStr {
    pub fn inner_as_ref(&self) -> Option<&str> {
        self.inner.as_ref().map(|val| val.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_serialize_opt_arc_str() {
        let opt_none = OptionArcStr { inner: None };
        let opt_some = OptionArcStr {
            inner: Some("something".into()),
        };

        let opt_none_serial = serde_json::to_string(&opt_none).unwrap();
        let opt_some_serial = serde_json::to_string(&opt_some).unwrap();

        assert_eq!(opt_none_serial, "null");
        assert_eq!(opt_some_serial, r#""something""#);

        let opt_none_deserial: OptionArcStr = serde_json::from_str(&opt_none_serial).unwrap();
        let opt_some_deserial: OptionArcStr = serde_json::from_str(&opt_some_serial).unwrap();

        assert_eq!(opt_none_deserial.inner, None);
        assert_eq!(opt_some_deserial.inner, Some("something".into()));
    }
}
