use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorInfo {
    pub code: u16,
    pub name: &'static str,
    pub http: u16,
    pub message: &'static str,
    pub action: &'static str,
    pub retryable: bool,
}

pub struct ErrorCodes {
    codes: HashMap<u16, ApiErrorInfo>,
}

impl ErrorCodes {
    pub fn new() -> Self {
        let mut codes = HashMap::new();
        
        // Auth errors
        codes.insert(1001, ApiErrorInfo {
            code: 1001,
            name: "AuthInvalidCredentials",
            http: 401,
            message: "用户名或密码错误",
            action: "请检查输入的凭证",
            retryable: true,
        });
        
        codes.insert(1002, ApiErrorInfo {
            code: 1002,
            name: "AuthSessionExpired",
            http: 401,
            message: "会话已过期，请重新登录",
            action: "重新登录",
            retryable: true,
        });
        
        codes.insert(1003, ApiErrorInfo {
            code: 1003,
            name: "AuthDeviceRevoked",
            http: 401,
            message: "设备已被撤销",
            action: "请重新绑定设备",
            retryable: false,
        });
        
        codes.insert(1004, ApiErrorInfo {
            code: 1004,
            name: "AuthPermissionDenied",
            http: 403,
            message: "权限不足",
            action: "请检查当前角色权限",
            retryable: false,
        });
        
        // Network errors
        codes.insert(2001, ApiErrorInfo {
            code: 2001,
            name: "NetworkNotFound",
            http: 404,
            message: "资源不存在",
            action: "请检查资源是否存在",
            retryable: false,
        });
        
        codes.insert(2002, ApiErrorInfo {
            code: 2002,
            name: "NetworkConflict",
            http: 409,
            message: "资源冲突",
            action: "请检查数据冲突",
            retryable: true,
        });
        
        // Database errors
        codes.insert(3001, ApiErrorInfo {
            code: 3001,
            name: "DatabaseConnectionFailed",
            http: 503,
            message: "数据库连接失败",
            action: "请检查数据库服务状态",
            retryable: true,
        });
        
        codes.insert(3002, ApiErrorInfo {
            code: 3002,
            name: "DatabaseIntegrityError",
            http: 400,
            message: "数据库完整性错误",
            action: "请检查数据格式",
            retryable: false,
        });
        
        codes.insert(3003, ApiErrorInfo {
            code: 3003,
            name: "ContractMismatch",
            http: 500,
            message: "契约版本不兼容",
            action: "请升级到最新版本",
            retryable: false,
        });
        
        // File errors
        codes.insert(4001, ApiErrorInfo {
            code: 4001,
            name: "FileNotFound",
            http: 404,
            message: "文件不存在",
            action: "请检查文件路径",
            retryable: false,
        });
        
        codes.insert(4002, ApiErrorInfo {
            code: 4002,
            name: "FileUploadFailed",
            http: 500,
            message: "文件上传失败",
            action: "请重试或检查网络",
            retryable: true,
        });
        
        // Sync errors
        codes.insert(5001, ApiErrorInfo {
            code: 5001,
            name: "SyncFailed",
            http: 500,
            message: "同步失败",
            action: "请检查网络连接并重试",
            retryable: true,
        });
        
        codes.insert(5002, ApiErrorInfo {
            code: 5002,
            name: "SyncCursorInvalid",
            http: 400,
            message: "同步游标无效",
            action: "请清除本地缓存后重新同步",
            retryable: false,
        });
        
        Self { codes }
    }
    
    pub fn get(&self, code: u16) -> Option<&ApiErrorInfo> {
        self.codes.get(&code)
    }
    
    pub fn all(&self) -> Vec<&ApiErrorInfo> {
        self.codes.values().collect()
    }
}

impl Default for ErrorCodes {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("错误 {code} ({name}): {message}")]
    Basic {
        code: u16,
        name: &'static str,
        http: u16,
        message: String,
        action: &'static str,
        retryable: bool,
    },
    
    #[error("契约版本不匹配: 期望 {expected}, 实际 {actual}")]
    ContractMismatch {
        expected: String,
        actual: String,
    },
    
    #[error("未知错误: {0}")]
    Unknown(String),
}

impl ApiError {
    pub fn new(code: u16, message: impl Into<String>) -> Self {
        let codes = ErrorCodes::new();
        if let Some(info) = codes.get(code) {
            Self::Basic {
                code: info.code,
                name: info.name,
                http: info.http,
                message: message.into(),
                action: info.action,
                retryable: info.retryable,
            }
        } else {
            panic!("未注册的错误码: {}", code)
        }
    }
    
    pub fn is_retryable(&self) -> bool {
        match self {
            ApiError::Basic { retryable, .. } => *retryable,
            _ => false,
        }
    }
    
    pub fn http_status(&self) -> u16 {
        match self {
            ApiError::Basic { http, .. } => *http,
            _ => 500,
        }
    }
    
    pub fn action(&self) -> &str {
        match self {
            ApiError::Basic { action, .. } => action,
            _ => "请稍后重试",
        }
    }
}
