use std::{fs, io::Write, path::Path};

use tauri::State;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    domain::workspace_portability::{
        ExportPortableWorkspaceInput, ImportPortableWorkspaceInput, PortableWorkspaceExportResult,
        PortableWorkspaceImportPreview, PortableWorkspaceImportResult,
    },
    error::AppResult,
    services::workspace_portability_service,
};

#[tauri::command]
pub async fn export_portable_workspace(
    state: State<'_, AppState>,
    input: ExportPortableWorkspaceInput,
) -> AppResult<Option<PortableWorkspaceExportResult>> {
    let document = workspace_portability_service::build_document(state.db(), &input).await?;
    let counts = workspace_portability_service::counts_for_document(&document);
    let redaction_count = document.redactions.len();
    let warnings = document.warnings.clone();
    let source = workspace_portability_service::serialize_document(&document)?;

    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export portable workspace")
            .set_file_name("postnot-workspace.postnot_workspace.json")
            .add_filter("PostNot workspace", &["json"])
            .save_file()
        else {
            return Ok(None);
        };
        write_replace_safe(&path, source.as_bytes())?;
        Ok(Some(PortableWorkspaceExportResult {
            file_path: path.to_string_lossy().to_string(),
            counts,
            redaction_count,
            warnings,
        }))
    })
    .await?
}

#[tauri::command]
pub fn inspect_portable_workspace(source: String) -> AppResult<PortableWorkspaceImportPreview> {
    workspace_portability_service::inspect_source(&source)
}

#[tauri::command]
pub async fn import_portable_workspace(
    state: State<'_, AppState>,
    input: ImportPortableWorkspaceInput,
) -> AppResult<PortableWorkspaceImportResult> {
    workspace_portability_service::import_source(
        state.db(),
        state.secret_store(),
        &input.source,
        input.include_open_drafts,
    )
    .await
}

fn write_replace_safe(path: &Path, contents: &[u8]) -> AppResult<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("postnot-workspace.json");
    let partial_path = parent.join(format!(".{file_name}.{}.partial", Uuid::new_v4()));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&partial_path)?;
        file.write_all(contents)?;
        file.sync_all()
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&partial_path);
        return Err(error.into());
    }
    match fs::rename(&partial_path, path) {
        Ok(()) => Ok(()),
        Err(error)
            if path.exists()
                && matches!(
                    error.kind(),
                    std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
                ) =>
        {
            let backup_path = parent.join(format!(".{file_name}.{}.previous", Uuid::new_v4()));
            if let Err(error) = fs::rename(path, &backup_path) {
                let _ = fs::remove_file(&partial_path);
                return Err(error.into());
            }
            match fs::rename(&partial_path, path) {
                Ok(()) => {
                    let _ = fs::remove_file(backup_path);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup_path, path);
                    let _ = fs::remove_file(&partial_path);
                    Err(error.into())
                }
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&partial_path);
            Err(error.into())
        }
    }
}
