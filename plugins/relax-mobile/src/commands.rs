use serde_json::Value;
use tauri::{command, AppHandle, Runtime, State};

use crate::Mobile;

pub(crate) fn relay<R: Runtime>(
    mobile: &State<'_, Mobile<R>>,
    cmd: &str,
    payload: Value,
) -> Result<Value, String> {
    #[cfg(target_os = "android")]
    {
        let handle = mobile
            .0
            .as_ref()
            .ok_or_else(|| "MOBILE_NOT_READY".to_string())?;
        handle
            .run_mobile_plugin::<Value>(cmd, payload)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (mobile, cmd, payload);
        Err("DESKTOP_UNSUPPORTED".to_string())
    }
}

#[command]
pub fn app_info<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "app_info", Value::Null)
}

#[command]
pub fn network_type<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "network_type", Value::Null)
}

#[command]
pub fn ensure_notification_channel<R: Runtime>(
    _app: AppHandle<R>,
    mobile: State<'_, Mobile<R>>,
) -> Result<Value, String> {
    relay(&mobile, "ensure_notification_channel", Value::Null)
}

#[command]
pub fn request_notifications<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "request_notifications", Value::Null)
}

#[command]
pub fn request_exact_alarm<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "request_exact_alarm", Value::Null)
}

#[command]
pub fn request_ignore_battery<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "request_ignore_battery", Value::Null)
}

#[command]
pub fn schedule_alarm<R: Runtime>(
    _app: AppHandle<R>,
    mobile: State<'_, Mobile<R>>,
    payload: Value,
) -> Result<Value, String> {
    relay(&mobile, "schedule_alarm", payload)
}

#[command]
pub fn cancel_alarm<R: Runtime>(
    _app: AppHandle<R>,
    mobile: State<'_, Mobile<R>>,
    payload: Value,
) -> Result<Value, String> {
    relay(&mobile, "cancel_alarm", payload)
}

#[command]
pub fn reschedule_all<R: Runtime>(
    _app: AppHandle<R>,
    mobile: State<'_, Mobile<R>>,
    payload: Value,
) -> Result<Value, String> {
    relay(&mobile, "reschedule_all", payload)
}

#[command]
pub fn list_photos<R: Runtime>(
    _app: AppHandle<R>,
    mobile: State<'_, Mobile<R>>,
    payload: Value,
) -> Result<Value, String> {
    relay(&mobile, "list_photos", payload)
}

#[command]
pub fn schedule_photo_sync<R: Runtime>(
    _app: AppHandle<R>,
    mobile: State<'_, Mobile<R>>,
    payload: Value,
) -> Result<Value, String> {
    relay(&mobile, "schedule_photo_sync", payload)
}

#[command]
pub fn start_sync_service<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "start_sync_service", Value::Null)
}

#[command]
pub fn stop_sync_service<R: Runtime>(_app: AppHandle<R>, mobile: State<'_, Mobile<R>>) -> Result<Value, String> {
    relay(&mobile, "stop_sync_service", Value::Null)
}
