mod diagnostics;
mod error;
mod loopback_proxy;
mod models;
mod paths;
mod runtime;
mod secure_store;
mod sparkle;
mod updater;

use std::sync::Arc;

use tokio::sync::Mutex;

use error::{CommandError, CommandResult, DesktopError};
use models::{DiagnosticsResult, RuntimeStatus, UpdateChannel, UpdatePhase, UpdateStatus};
use paths::DesktopPaths;
use runtime::RuntimeManager;
use secure_store::SecureStore;
use tauri::{
    menu::{Menu, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Emitter, Manager, State,
};
use updater::UpdateManager;

struct AppState {
    paths: DesktopPaths,
    runtime: Arc<RuntimeManager>,
    updates: Arc<UpdateManager>,
    secure: SecureStore,
    operation: Arc<Mutex<()>>,
}

#[tauri::command]
async fn runtime_status(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    Ok(state.runtime.status().await)
}

#[tauri::command]
async fn runtime_start(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    let api_key = state
        .secure
        .get("deepseek-api-key")
        .map_err(CommandError::from)?;
    state.runtime.start(api_key).await.map_err(Into::into)
}

#[tauri::command]
async fn runtime_stop(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    state.runtime.stop().await.map_err(Into::into)
}

#[tauri::command]
async fn runtime_restart(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    let api_key = state
        .secure
        .get("deepseek-api-key")
        .map_err(CommandError::from)?;
    state.runtime.restart(api_key).await.map_err(Into::into)
}

#[tauri::command]
async fn runtime_update_check(
    state: State<'_, AppState>,
    channel: UpdateChannel,
) -> CommandResult<UpdateStatus> {
    let runtime = state.runtime.status().await;
    state
        .updates
        .check_runtime(channel, &runtime.version, runtime.rollback_available)
        .await
        .map_err(Into::into)
}

#[tauri::command]
async fn runtime_update_install(
    state: State<'_, AppState>,
    version: String,
    channel: UpdateChannel,
) -> CommandResult<UpdateStatus> {
    let _operation = state.operation.lock().await;
    let previous = state.runtime.status().await;
    let was_running = matches!(
        previous.state,
        models::RuntimeState::Running | models::RuntimeState::Starting
    );
    if was_running {
        state
            .runtime
            .stop_with_operation_held()
            .await
            .map_err(CommandError::from)?;
    }

    let result = state
        .updates
        .install_runtime_with_operation_held(channel, &version, &previous.version)
        .await;
    if let Err(error) = result {
        if was_running {
            let api_key = state.secure.get("deepseek-api-key").ok().flatten();
            let _ = state.runtime.start_with_operation_held(api_key).await;
        }
        return Err(error.into());
    }

    if was_running {
        let api_key = state
            .secure
            .get("deepseek-api-key")
            .map_err(CommandError::from)?;
        if let Err(start_error) = state.runtime.start_with_operation_held(api_key).await {
            state
                .updates
                .rollback_runtime_with_operation_held()
                .await
                .map_err(CommandError::from)?;
            let rollback_key = state.secure.get("deepseek-api-key").ok().flatten();
            let _ = state.runtime.start_with_operation_held(rollback_key).await;
            return Err(DesktopError::Runtime(format!(
                "updated runtime failed its health check and was rolled back: {start_error}"
            ))
            .into());
        }
    }

    Ok(result.expect("checked above"))
}

#[tauri::command]
async fn app_update_check(
    state: State<'_, AppState>,
    channel: UpdateChannel,
) -> CommandResult<UpdateStatus> {
    state.updates.check_app(channel).await.map_err(Into::into)
}

#[tauri::command]
async fn app_update_install(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: UpdateChannel,
) -> CommandResult<UpdateStatus> {
    let feed_url = state.updates.appcast_url(channel);
    sparkle::check_for_updates(&app, &feed_url)
        .await
        .map_err(CommandError::from)?;
    Ok(UpdateStatus {
        component: "desktop".into(),
        current_version: env!("CARGO_PKG_VERSION").into(),
        available_version: None,
        channel,
        phase: UpdatePhase::HandedOff,
        progress: 0,
        requires_restart: true,
        error_code: None,
        rollback_available: false,
        release_notes: None,
    })
}

#[tauri::command]
fn secure_get(state: State<'_, AppState>, key: String) -> CommandResult<Option<String>> {
    state.secure.get(&key).map_err(Into::into)
}

#[tauri::command]
fn secure_set(state: State<'_, AppState>, key: String, value: String) -> CommandResult<()> {
    state.secure.set(&key, &value).map_err(Into::into)
}

#[tauri::command]
fn secure_delete(state: State<'_, AppState>, key: String) -> CommandResult<()> {
    state.secure.delete(&key).map_err(Into::into)
}

#[tauri::command]
async fn diagnostics_export(state: State<'_, AppState>) -> CommandResult<DiagnosticsResult> {
    let status = state.runtime.status().await;
    diagnostics::export(&state.paths, &status).map_err(Into::into)
}

#[tauri::command]
async fn runtime_rollback(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    let _operation = state.operation.lock().await;
    let previous = state.runtime.status().await;
    let was_running = matches!(
        previous.state,
        models::RuntimeState::Running | models::RuntimeState::Starting
    );
    if was_running {
        state
            .runtime
            .stop_with_operation_held()
            .await
            .map_err(CommandError::from)?;
    }
    state
        .updates
        .rollback_runtime_with_operation_held()
        .await
        .map_err(CommandError::from)?;

    if was_running {
        let api_key = state.secure.get("deepseek-api-key").ok().flatten();
        match state.runtime.start_with_operation_held(api_key).await {
            Ok(status) => Ok(status),
            Err(error) => {
                let _ = state.updates.rollback_runtime_with_operation_held().await;
                let restore_key = state.secure.get("deepseek-api-key").ok().flatten();
                let _ = state.runtime.start_with_operation_held(restore_key).await;
                Err(DesktopError::Runtime(format!(
                    "rollback runtime failed its health check; the original version was restored: {error}"
                ))
                .into())
            }
        }
    } else {
        Ok(state.runtime.status().await)
    }
}

#[tauri::command]
fn logs_clear(state: State<'_, AppState>) -> CommandResult<()> {
    diagnostics::clear_logs(&state.paths).map_err(Into::into)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            let start = MenuItemBuilder::with_id("runtime-start", "Start Runtime")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(handle)?;
            let restart = MenuItemBuilder::with_id("runtime-restart", "Restart Runtime")
                .accelerator("CmdOrCtrl+Shift+R")
                .build(handle)?;
            let updates = MenuItemBuilder::with_id("updates", "Check for All Updates...")
                .accelerator("CmdOrCtrl+U")
                .build(handle)?;
            let settings = MenuItemBuilder::with_id("settings", "Settings...")
                .accelerator("CmdOrCtrl+Comma")
                .build(handle)?;
            let harness_menu = SubmenuBuilder::new(handle, "Harness")
                .item(&start)
                .item(&restart)
                .separator()
                .item(&updates)
                .item(&settings)
                .build()?;
            let menu = Menu::default(handle)?;
            menu.append(&harness_menu)?;
            app.set_menu(menu)?;

            let resource_dir = app.path().resource_dir().ok();
            let paths = DesktopPaths::discover(resource_dir)?;
            let file_appender = tracing_appender::rolling::daily(&paths.logs, "desktop.log");
            tracing_subscriber::fmt()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(false)
                .try_init()
                .ok();

            let operation = Arc::new(Mutex::new(()));
            let runtime = Arc::new(RuntimeManager::new(paths.clone(), operation.clone()));
            let updates = Arc::new(UpdateManager::new(paths.clone()));
            app.manage(AppState {
                paths,
                runtime,
                updates,
                secure: SecureStore,
                operation,
            });
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("desktop-menu", event.id().as_ref());
        })
        .invoke_handler(tauri::generate_handler![
            runtime_status,
            runtime_start,
            runtime_stop,
            runtime_restart,
            runtime_update_check,
            runtime_update_install,
            app_update_check,
            app_update_install,
            secure_get,
            secure_set,
            secure_delete,
            diagnostics_export,
            runtime_rollback,
            logs_clear,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DeepSeek Harness Desktop");
}
