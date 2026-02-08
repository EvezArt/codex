use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Covenant {
    pub version: String,
    pub scopes: Vec<CovenantScope>,
}

#[derive(Debug, Deserialize)]
pub struct CovenantScope {
    pub name: String,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum CovenantAction {
    ProposalExecCommand,
    ProposalApplyPatch,
    InterventionExecApproval,
    InterventionPatchApproval,
    InterventionUserShell,
}

impl CovenantAction {
    pub fn as_capability(self) -> &'static str {
        match self {
            CovenantAction::ProposalExecCommand => "proposal.exec_command",
            CovenantAction::ProposalApplyPatch => "proposal.apply_patch",
            CovenantAction::InterventionExecApproval => "intervention.exec_approval",
            CovenantAction::InterventionPatchApproval => "intervention.patch_approval",
            CovenantAction::InterventionUserShell => "intervention.user_shell",
        }
    }
}

impl Covenant {
    pub fn allows(&self, scope: &str, capability: &str) -> bool {
        self.scopes.iter().any(|scope_entry| {
            scope_entry.name == scope
                && scope_entry
                    .capabilities
                    .iter()
                    .any(|entry| entry == capability)
        })
    }
}

pub async fn load_covenant(cwd: &Path) -> anyhow::Result<Covenant> {
    let covenant_path = find_covenant_path(cwd)
        .await
        .ok_or_else(|| anyhow::anyhow!("covenant.json not found from {}", cwd.display()))?;
    let contents = tokio::fs::read_to_string(&covenant_path).await?;
    let covenant = serde_json::from_str(&contents)?;
    Ok(covenant)
}

async fn find_covenant_path(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(cwd);
    while let Some(path) = current {
        let candidate = path.join("covenant.json");
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return Some(candidate);
        }
        current = path.parent();
    }
    None
}
