use tauri::{
    plugin::{Builder, PluginApi, PluginHandle, TauriPlugin},
    Manager, Runtime,
};

mod commands;

const PLUGIN_IDENTIFIER: &str = "dev.relax.mobile";

pub struct Mobile<R: Runtime>(Option<PluginHandle<R>>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("relax-mobile")
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::network_type,
            commands::ensure_notification_channel,
            commands::request_notifications,
            commands::request_exact_alarm,
            commands::request_ignore_battery,
            commands::schedule_alarm,
            commands::cancel_alarm,
            commands::reschedule_all,
            commands::list_photos,
            commands::schedule_photo_sync,
            commands::start_sync_service,
            commands::stop_sync_service,
        ])
        .setup(|app, api| {
            register(app, &api)?;
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
fn register<R: Runtime>(app: &tauri::AppHandle<R>, api: &PluginApi<R, ()>) -> tauri::Result<()> {
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "RelaxMobilePlugin")?;
    app.manage(Mobile(Some(handle)));
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn register<R: Runtime>(app: &tauri::AppHandle<R>, _api: &PluginApi<R, ()>) -> tauri::Result<()> {
    app.manage(Mobile::<R>(None));
    Ok(())
}
