use rusqlite::{Connection, params};
use thiserror::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{Utc};

#[derive(Error, Debug)]
pub enum ChatError {
    #[error("数据库错误: {0}")]
    Database(String),
    #[error("消息不存在")]
    MessageNotFound,
}

pub struct ChatModule {
    db: Connection,
}

impl ChatModule {
    pub fn new(db_path: &str) -> Result<Self, ChatError> {
        let conn = Connection::open(db_path).map_err(|e| ChatError::Database(e.to_string()))?;
        Ok(Self { db: conn })
    }

    pub fn send_message(&self, content: &str, at_users: &[String], reply_to: Option<&str>, sender_id: &str) -> Result<String, ChatError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc().to_rfc3339();
        
        self.db.execute(
            "INSERT INTO messages (id, content, at_users, reply_to, sender_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, content, serde_json::to_string(at_users).unwrap_or_else(|_| "[]".to_string()), reply_to, sender_id, now],
        ).map_err(|e| ChatError::Database(e.to_string()))?;
        
        Ok(id)
    }

    pub fn list_messages(&self, cursor: Option<&str>, limit: i64) -> Result<Vec<Message>, ChatError> {
        let mut query = "SELECT id, content, at_users, reply_to, sender_id, created_at FROM messages".to_string();
        
        if let Some(cursor_val) = cursor {
            query.push_str(" WHERE created_at > ?1");
        }
        
        query.push_str(" ORDER BY created_at DESC LIMIT ?2");
        
        let mut stmt = self.db.prepare(&query).map_err(|e| ChatError::Database(e.to_string()))?;
        
        let messages = if let Some(cursor_val) = cursor {
            stmt.query_map(params![cursor_val, limit], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    at_users: serde_json::from_str::<Vec<String>>(row.get::<_, String>(2)?).unwrap_or_else(|_| vec![]),
                    reply_to: row.get(3)?,
                    sender_id: row.get(4)?,
                    created_at: row.get::<_, String>(5)?,
                })
            }).map_err(|e| ChatError::Database(e.to_string()))?
            .filter_map(|m| m.ok())
            .collect()
        } else {
            stmt.query_map(params![limit], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    at_users: serde_json::from_str::<Vec<String>>(row.get::<_, String>(2)?).unwrap_or_else(|_| vec![]),
                    reply_to: row.get(3)?,
                    sender_id: row.get(4)?,
                    created_at: row.get::<_, String>(5)?,
                })
            }).map_err(|e| ChatError::Database(e.to_string()))?
            .filter_map(|m| m.ok())
            .collect()
        };
        
        Ok(messages)
    }

    pub fn mark_read(&self, message_ids: &[String]) -> Result<usize, ChatError> {
        for id in message_ids {
            let now = Utc::now().naive_utc().to_rfc3339();
            self.db.execute("UPDATE messages SET read_at = ?1 WHERE id = ?2", params![now, id]).ok();
        }
        Ok(message_ids.len())
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>, ChatError> {
        let mut stmt = self.db.prepare("SELECT id, content, at_users, reply_to, sender_id, created_at FROM messages WHERE id = ?1")
            .map_err(|e| ChatError::Database(e.to_string()))?;
        let message = stmt.query_row(params![id], |row| {
            Ok(Message {
                id: row.get(0)?,
                content: row.get(1)?,
                at_users: serde_json::from_str::<Vec<String>>(row.get::<_, String>(2)?).unwrap_or_else(|_| vec![]),
                reply_to: row.get(3)?,
                sender_id: row.get(4)?,
                created_at: row.get::<_, String>(5)?,
            })
        }).ok();
        Ok(message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub at_users: Vec<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    pub sender_id: String,
    pub created_at: String,
}
