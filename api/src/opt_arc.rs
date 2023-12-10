use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OptionArcStr {
    inner: Option<Arc<str>>,
}

impl From<OptionArcStr> for Option<Arc<str>> {
    fn from(value: OptionArcStr) -> Self {
        value.inner
    }
}

impl From<Option<Arc<str>>> for OptionArcStr {
    fn from(value: Option<Arc<str>>) -> Self {
        Self {
            inner: value.map(|str| str.into()),
        }
    }
}

impl From<Option<String>> for OptionArcStr {
    fn from(value: Option<String>) -> Self {
        Self {
            inner: value.map(|str| str.into()),
        }
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
