use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const COVENANT_FILE_NAME: &str = "covenant.json";
pub(crate) const SCOPE_APPLY_PATCH: &str = "apply_patch";
pub(crate) const SCOPE_EXEC: &str = "exec";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Covenant {
    pub(crate) version: String,
    pub(crate) scopes: Vec<CovenantScope>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct CovenantScope {
    pub(crate) name: String,
    pub(crate) capabilities: Vec<String>,
}

impl Covenant {
    pub(crate) fn allows_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|entry| entry.name == scope)
    }

    pub(crate) fn scopes_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(&self.scopes)?)
    }
}

pub(crate) async fn load_from_cwd(cwd: &Path) -> anyhow::Result<(Covenant, PathBuf)> {
    let covenant_path = find_covenant_path(cwd).await?;
    let raw = tokio::fs::read_to_string(&covenant_path).await?;
    let covenant = serde_json::from_str::<Covenant>(&raw)?;
    Ok((covenant, covenant_path))
}

async fn find_covenant_path(start: &Path) -> anyhow::Result<PathBuf> {
    let mut current = Some(start);
    while let Some(path) = current {
        let candidate = path.join(COVENANT_FILE_NAME);
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return Ok(candidate);
        }
        current = path.parent();
    }
    anyhow::bail!(
        "no {COVENANT_FILE_NAME} found when walking up from {}",
        start.display()
    )
}
