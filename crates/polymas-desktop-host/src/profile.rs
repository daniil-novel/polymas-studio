use crate::{
    NativeConnection,
    service::{CredentialVault, RunningService, ServiceRecord},
};
use polymas_core::{
    error::{Error, Result, require},
    storage::Store,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

pub struct NativeProfile {
    pub connection: Arc<NativeConnection>,
    pub record: ServiceRecord,
    pub reused: bool,
    pub warnings: Vec<String>,
}
fn cleanup(db: &mut Store, key: &str, vault: &dyn CredentialVault) -> Result<Vec<String>> {
    let Some(saved) = db.get::<Vec<String>>("desktop_credential_cleanup", key)? else {
        return Ok(vec![]);
    };
    let remaining: Vec<_> = saved
        .value
        .iter()
        .filter(|reference| vault.remove(reference).is_err())
        .cloned()
        .collect();
    if remaining != saved.value {
        db.transaction(|tx| {
            tx.put(
                "desktop_credential_cleanup",
                key,
                saved.revision,
                &remaining,
            )?;
            Ok(())
        })?;
    }
    Ok(if remaining.is_empty() {
        vec![]
    } else {
        vec![format!(
            "Не удалось очистить {} старых записей системного хранилища; повторим при следующем запуске.",
            remaining.len()
        )]
    })
}
impl NativeProfile {
    pub async fn connect(
        profile_dir: &Path,
        workspace: &Path,
        executable: &Path,
        capacity: usize,
        vault: &dyn CredentialVault,
    ) -> Result<Self> {
        require(
            profile_dir.is_absolute() && workspace.is_absolute(),
            "Профиль и рабочая область должны иметь абсолютные пути",
        )?;
        std::fs::create_dir_all(profile_dir)?;
        std::fs::create_dir_all(workspace)?;
        let startup_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(profile_dir.join("startup.lock"))?;
        fs2::FileExt::try_lock_exclusive(&startup_lock)
            .map_err(|_| Error::Conflict("Другое окно уже подключает службу".into()))?;
        let canonical = workspace.canonicalize()?;
        let key = format!(
            "{:x}",
            Sha256::digest(canonical.to_string_lossy().as_bytes())
        );
        let mut db = Store::open(&profile_dir.join("desktop.sqlite3"))?;
        let prior = db.get::<ServiceRecord>("desktop_services", &key)?;
        if let Some(saved) = &prior {
            match saved.value.reconnect(vault).await {
                Ok(connection) => {
                    connection
                        .request("/api/control", Some(json!({"action":"adopt"})))
                        .await
                        .map_err(|e| Error::Runtime(e.message))?;
                    return Ok(Self {
                        connection,
                        record: saved.value.clone(),
                        reused: true,
                        warnings: cleanup(&mut db, &key, vault)?,
                    });
                }
                Err(Error::Runtime(_)) => {}
                Err(error) => return Err(error),
            }
        }
        let service = RunningService::launch_unadopted(executable, &canonical, capacity).await?;
        let record = match service.remember(vault) {
            Ok(record) => record,
            Err(error) => {
                service.shutdown().await.ok();
                return Err(error);
            }
        };
        let revision = prior.as_ref().map(|s| s.revision).unwrap_or(0);
        let saved = db.transaction(|tx| {
            tx.put("desktop_services", &key, revision, &record)?;
            if let Some(prior) = &prior {
                let pending = tx.get::<Vec<String>>("desktop_credential_cleanup", &key)?;
                let rev = pending.as_ref().map(|s| s.revision).unwrap_or(0);
                let mut references = pending.map(|s| s.value).unwrap_or_default();
                if !references.contains(&prior.value.credential_ref) {
                    references.push(prior.value.credential_ref.clone());
                }
                require(
                    references.len() <= 1000,
                    "Очередь очистки credentials заполнена; проверьте системное хранилище",
                )?;
                tx.put("desktop_credential_cleanup", &key, rev, &references)?;
            }
            Ok(())
        });
        if let Err(error) = saved {
            vault.remove(&record.credential_ref).ok();
            service.shutdown().await.ok();
            return Err(error);
        }
        let connection = service.connection();
        connection
            .request("/api/control", Some(json!({"action":"adopt"})))
            .await
            .map_err(|e| Error::Runtime(e.message))?;
        // Endpoint and vault reference are committed before releasing startup.
        let warnings = cleanup(&mut db, &key, vault)?;
        Ok(Self {
            connection,
            record,
            reused: false,
            warnings,
        })
    }
}
