//! 数据验证模块

use thiserror::Error;

#[derive(Error, Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

#[derive(Debug)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

impl LoginRequest {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        
        if self.username.len() < 3 {
            errors.push(ValidationError {
                field: "username".to_string(),
                message: "用户名至少 3 个字符".to_string(),
            });
        }
        
        if self.username.len() > 64 {
            errors.push(ValidationError {
                field: "username".to_string(),
                message: "用户名最多 64 个字符".to_string(),
            });
        }
        
        if self.password.len() < 8 {
            errors.push(ValidationError {
                field: "password".to_string(),
                message: "密码至少 8 个字符".to_string(),
            });
        }
        
        if self.password.len() > 128 {
            errors.push(ValidationError {
                field: "password".to_string(),
                message: "密码最多 128 个字符".to_string(),
            });
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
