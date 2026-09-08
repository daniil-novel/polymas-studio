use crate::artifacts::{Artifact, Artifacts, MAX_ARTIFACT_BYTES, validate_hash};
use crate::error::{Error, Result, require};
use crate::registry::now;
use crate::storage::Store;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupManifest {
    pub format: String,
    pub version: u32,
    pub created_at: String,
    pub database_hash: String,
    pub database_size: u64,
    pub blobs: BTreeMap<String, u64>,
}
pub struct Backup;
impl Backup {
    /// Publish only a complete snapshot. The caller serializes workspace writes
    /// while SQLite's backup API captures the snapshot; blobs are immutable.
    pub fn create(db: &Store, artifacts: &Artifacts, destination: &Path) -> Result<BackupManifest> {
        let stage = stage_for(destination)?;
        let database = stage.path().join("state.sqlite3");
        db.backup(&database)?;
        let entries = referenced_artifacts(&database)?;
        let mut blobs = BTreeMap::new();
        std::fs::create_dir(stage.path().join("artifacts"))?;
        for artifact in entries {
            if blobs.contains_key(&artifact.sha256) {
                continue;
            }
            let bytes = artifacts.read(&artifact)?;
            write_synced(
                &stage.path().join("artifacts").join(&artifact.sha256),
                &bytes,
            )?;
            blobs.insert(artifact.sha256, artifact.size);
        }
        let (database_hash, database_size) = hash_file(std::fs::File::open(&database)?)?;
        let manifest = BackupManifest {
            format: "polymas-backup".into(),
            version: 1,
            created_at: now(),
            database_hash,
            database_size,
            blobs,
        };
        write_synced(
            &stage.path().join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest)?,
        )?;
        std::fs::rename(stage.path(), destination)?;
        Ok(manifest)
    }
    /// Restore into a new directory; never modify an open/existing workspace.
    /// No caller-controlled path from a manifest is ever joined to disk paths.
    pub fn restore(source: &Path, destination: &Path) -> Result<BackupManifest> {
        let stage = stage_for(destination)?;
        let dir = cap_std::fs::Dir::open_ambient_dir(source, cap_std::ambient_authority())?;
        let file = dir.open("manifest.json")?;
        require(
            file.metadata()?.len() <= 16 * 1024 * 1024,
            "Manifest слишком большой",
        )?;
        let mut bytes = Vec::new();
        file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        require(bytes.len() <= 16 * 1024 * 1024, "Manifest слишком большой")?;
        let manifest: BackupManifest = serde_json::from_slice(&bytes)?;
        require(
            manifest.format == "polymas-backup" && manifest.version == 1,
            "Неизвестный формат backup",
        )?;
        validate_hash(&manifest.database_hash)?;
        require(
            manifest.blobs.len() <= 100000 && manifest.database_size <= 64 * 1024 * 1024 * 1024,
            "Backup превышает поддерживаемый размер",
        )?;
        let mut database = dir.open("state.sqlite3")?;
        require(
            database.metadata()?.len() == manifest.database_size,
            "Размер snapshot не совпадает",
        )?;
        let staged_database = stage.path().join("state.sqlite3");
        let mut output = std::fs::File::create(&staged_database)?;
        std::io::copy(&mut database, &mut output)?;
        output.sync_all()?;
        drop(output);
        let (hash, size) = hash_file(std::fs::File::open(&staged_database)?)?;
        require(
            hash == manifest.database_hash && size == manifest.database_size,
            "Snapshot повреждён: SHA-256 не совпадает",
        )?;
        let entries = referenced_artifacts(&staged_database)?;
        let referenced: BTreeMap<_, _> = entries.into_iter().map(|a| (a.sha256, a.size)).collect();
        require(
            referenced == manifest.blobs,
            "Manifest не совпадает со ссылками snapshot",
        )?;
        std::fs::create_dir(stage.path().join("artifacts"))?;
        for (hash, size) in &manifest.blobs {
            validate_hash(hash)?;
            require(*size <= MAX_ARTIFACT_BYTES as u64, "Blob слишком большой")?;
            let mut file = dir.open(format!("artifacts/{hash}"))?;
            require(file.metadata()?.len() == *size, "Размер blob не совпадает")?;
            let target = stage.path().join("artifacts").join(hash);
            let mut output = std::fs::File::create(&target)?;
            std::io::copy(&mut file, &mut output)?;
            output.sync_all()?;
            drop(output);
            let (actual, actual_size) = hash_file(std::fs::File::open(&target)?)?;
            require(actual == *hash && actual_size == *size, "Blob повреждён")?;
        }
        std::fs::rename(stage.path(), destination)?;
        Ok(manifest)
    }
}
fn stage_for(destination: &Path) -> Result<tempfile::TempDir> {
    require(
        destination.is_absolute() && !destination.exists(),
        "Выберите новый абсолютный путь",
    )?;
    let parent = destination
        .parent()
        .ok_or_else(|| Error::Validation("Нет родительской папки".into()))?;
    require(parent.is_dir(), "Родительская папка не существует")?;
    Ok(tempfile::Builder::new()
        .prefix(".polymas-backup-")
        .tempdir_in(parent)?)
}
fn referenced_artifacts(database: &Path) -> Result<Vec<Artifact>> {
    let connection = rusqlite::Connection::open_with_flags(
        database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    require(integrity == "ok", "Нарушена целостность snapshot")?;
    let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    require(version == 2, "Несовместимая версия snapshot")?;
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM entities WHERE collection='artifacts'",
        [],
        |r| r.get(0),
    )?;
    require(count <= 100000, "Backup поддерживает до 100000 артефактов")?;
    let mut statement =
        connection.prepare("SELECT document FROM entities WHERE collection='artifacts'")?;
    let rows = statement.query_map([], |r| r.get::<_, String>(0))?;
    rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
}
fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn hash_file(mut reader: impl Read) -> Result<(String, u64)> {
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        size += n as u64;
    }
    Ok((format!("{:x}", hash.finalize()), size))
}
