use crate::error::{Error, Result, require};
use crate::registry::{id, now};
use crate::storage::{EventInput, Store};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const MAX_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactInput {
    pub name: String,
    pub media_type: String,
    pub agent_id: String,
    pub task_id: String,
    pub attempt: u32,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub sha256: String,
    pub size: u64,
    pub created_at: String,
    #[serde(flatten)]
    pub input: ArtifactInput,
    pub accepted: bool,
}
impl std::ops::Deref for Artifact {
    type Target = ArtifactInput;
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}
pub struct Artifacts {
    root: PathBuf,
}
impl Artifacts {
    pub fn new(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn publish(&self, db: &mut Store, input: ArtifactInput, bytes: &[u8]) -> Result<Artifact> {
        require(
            bytes.len() <= MAX_ARTIFACT_BYTES,
            "Артефакт превышает 64 МиБ",
        )?;
        require(
            !input.name.trim().is_empty()
                && input.name.len() <= 240
                && !input.name.chars().any(char::is_control),
            "Некорректное имя артефакта",
        )?;
        require(
            !input.media_type.is_empty()
                && input.media_type.len() <= 120
                && input.attempt > 0
                && !input.agent_id.is_empty()
                && !input.task_id.is_empty(),
            "Артефакт требует автора и attempt",
        )?;
        let artifact = Artifact {
            id: id("artifact"),
            sha256: digest(bytes),
            size: bytes.len() as u64,
            created_at: now(),
            input,
            accepted: false,
        };
        let destination = self.root.join(&artifact.sha256);
        if destination.exists() {
            self.read(&artifact)?;
        } else {
            let mut temp = tempfile::NamedTempFile::new_in(&self.root)?;
            temp.write_all(bytes)?;
            temp.as_file().sync_all()?;
            match temp.persist_noclobber(&destination) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.read(&artifact)?;
                }
                Err(error) => return Err(Error::Runtime(error.error.to_string())),
            }
        }
        // An interrupted database transaction may leave an unreferenced blob,
        // but never a visible artifact whose file has not been durably written.
        db.transaction(|tx| {
            tx.put("artifacts",&artifact.id,0,&artifact)?;
            tx.emit(EventInput::new("artifact.published",json!({"artifactId":artifact.id,"taskId":artifact.task_id,"sha256":artifact.sha256,"partial":artifact.partial})).agent(&artifact.agent_id))?;
            Ok(artifact)
        })
    }
    pub fn read(&self, artifact: &Artifact) -> Result<Vec<u8>> {
        validate_hash(&artifact.sha256)?;
        let dir = cap_std::fs::Dir::open_ambient_dir(&self.root, cap_std::ambient_authority())?;
        let file = dir.open(&artifact.sha256)?;
        require(
            file.metadata()?.is_file() && file.metadata()?.len() <= MAX_ARTIFACT_BYTES as u64,
            "Некорректный blob",
        )?;
        let mut bytes = Vec::new();
        file.take(MAX_ARTIFACT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        require(
            bytes.len() as u64 == artifact.size && digest(&bytes) == artifact.sha256,
            "Hash или размер артефакта не совпадает",
        )?;
        Ok(bytes)
    }
    pub fn import_file(
        &self,
        db: &mut Store,
        input: ArtifactInput,
        workspace: &Path,
        relative_path: &str,
    ) -> Result<Artifact> {
        require(
            !relative_path.is_empty()
                && !relative_path.contains([':', '\\'])
                && Path::new(relative_path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "Путь должен оставаться внутри workspace агента",
        )?;
        let dir = cap_std::fs::Dir::open_ambient_dir(workspace, cap_std::ambient_authority())?;
        let file = dir.open(relative_path)?;
        require(
            file.metadata()?.is_file() && file.metadata()?.len() <= MAX_ARTIFACT_BYTES as u64,
            "Файл отсутствует или слишком большой",
        )?;
        let mut bytes = Vec::new();
        file.take(MAX_ARTIFACT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        self.publish(db, input, &bytes)
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn validate_hash(hash: &str) -> Result<()> {
    require(
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "Некорректный SHA-256",
    )
}
