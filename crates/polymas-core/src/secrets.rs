use crate::error::{Error, Result, require};
use crate::registry::id;
use zeroize::Zeroizing;

/// Deliberately has no Serialize or Display implementation.
pub struct Secret(Zeroizing<String>);
impl Secret {
    pub fn new(value: String) -> Result<Self> {
        require(
            !value.is_empty() && value.len() <= 16_384,
            "Секрет должен содержать от 1 до 16384 байт",
        )?;
        Ok(Self(Zeroizing::new(value)))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

#[derive(Clone)]
pub struct OsVault {
    service: String,
}
impl OsVault {
    pub fn new(workspace_id: &str) -> Result<Self> {
        require(
            !workspace_id.is_empty()
                && workspace_id.len() <= 100
                && workspace_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "Некорректный namespace секретов",
        )?;
        Ok(Self {
            service: format!("PolyMASStudio/{workspace_id}"),
        })
    }
    fn entry(&self, reference: &str) -> Result<keyring::Entry> {
        let uuid = reference
            .strip_prefix("secret_")
            .ok_or_else(|| Error::Validation("Некорректная ссылка на секрет".into()))?;
        uuid::Uuid::parse_str(uuid)
            .map_err(|_| Error::Validation("Некорректный UUID секрета".into()))?;
        keyring::Entry::new(&self.service, reference)
            .map_err(|_| Error::Runtime("Системное хранилище секретов недоступно".into()))
    }
    pub fn save(&self, secret: &Secret) -> Result<String> {
        let reference = id("secret");
        self.entry(&reference)?
            .set_password(secret.expose())
            .map_err(|_| Error::Runtime("Секрет не сохранён в системном хранилище".into()))?;
        Ok(reference)
    }
    pub fn load(&self, reference: &str) -> Result<Secret> {
        Secret::new(self.entry(reference)?.get_password().map_err(|_| {
            Error::Denied(
                "Секрет отсутствует или доступ отозван; настройте подключение заново".into(),
            )
        })?)
    }
    pub fn remove(&self, reference: &str) -> Result<()> {
        self.entry(reference)?
            .delete_credential()
            .map_err(|_| Error::Runtime("Не удалось удалить указанный секрет".into()))
    }
}
