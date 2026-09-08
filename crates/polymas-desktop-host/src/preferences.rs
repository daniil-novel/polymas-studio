use polymas_core::{
    error::{Result, require},
    storage::Store,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    Background,
    Stop,
}

pub struct Preferences {
    path: PathBuf,
}
impl Preferences {
    pub fn open(path: &Path) -> Result<Self> {
        require(path.is_absolute(), "Путь настроек должен быть абсолютным")?;
        Store::open(path)?;
        Ok(Self { path: path.into() })
    }
    pub fn close_behavior(&self) -> Result<Option<CloseBehavior>> {
        Ok(Store::open(&self.path)?
            .get::<Option<CloseBehavior>>("desktop_preferences", "close_behavior")?
            .and_then(|s| s.value))
    }
    pub fn set_close_behavior(&self, value: Option<CloseBehavior>) -> Result<()> {
        Store::open(&self.path)?.transaction(|tx| {
            let revision = tx
                .get::<Option<CloseBehavior>>("desktop_preferences", "close_behavior")?
                .map(|s| s.revision)
                .unwrap_or(0);
            tx.put("desktop_preferences", "close_behavior", revision, &value)?;
            Ok(())
        })
    }
}
