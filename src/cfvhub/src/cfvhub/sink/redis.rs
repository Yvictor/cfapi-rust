use super::{Formated, FormaterExt, SinkError, SinkExt};
use redis::{Client, Commands, Connection, RedisError};
use serde::Serialize;
use snafu::prelude::*;
use std::sync::Mutex;
use tracing::error;

#[derive(Debug, Snafu)]
pub enum RedisSinkError {
    #[snafu(display("Redis connection error: {}", source))]
    Connection { source: RedisError },
    #[snafu(display("Redis command error: {}", source))]
    Command { source: RedisError },
}

impl From<RedisSinkError> for SinkError {
    fn from(err: RedisSinkError) -> Self {
        match err {
            RedisSinkError::Connection { source } => {
                SinkError::RedisConnection { source }
            }
            RedisSinkError::Command { source } => {
                SinkError::RedisCommand { source }
            }
        }
    }
}

pub struct RedisSink {
    client: Client,
    connection: Mutex<Connection>,
    key_prefix: String,
    data_structure: RedisDataStructure,
}

#[derive(Debug, Clone)]
pub enum RedisDataStructure {
    List(String),      // Key name for LIST
    Stream(String),    // Key name for STREAM
    PubSub(String),    // Channel name for PUB/SUB
}

impl Default for RedisSink {
    fn default() -> Self {
        let redis_url = dotenvy::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let key_prefix = dotenvy::var("REDIS_KEY_PREFIX")
            .unwrap_or_else(|_| "cfvhub".to_string());
        let data_structure = dotenvy::var("REDIS_DATA_STRUCTURE")
            .unwrap_or_else(|_| "list".to_string());
        
        let data_structure = match data_structure.as_str() {
            "stream" => RedisDataStructure::Stream(format!("{}:stream", key_prefix)),
            "pubsub" => RedisDataStructure::PubSub(format!("{}:channel", key_prefix)),
            _ => RedisDataStructure::List(format!("{}:list", key_prefix)),
        };
        Self::new(&redis_url, key_prefix, data_structure).unwrap()
    }
}

impl RedisSink {
    pub fn new(url: &str, key_prefix: String, data_structure: RedisDataStructure) -> Result<Self, RedisSinkError> {
        let client = Client::open(url).context(ConnectionSnafu)?;
        let connection = client.get_connection().context(ConnectionSnafu)?;
        // tracing::info!("RedisSink new: {}, {}, {:?}", url, key_prefix, data_structure);
        Ok(Self {
            client,
            connection: Mutex::new(connection),
            key_prefix,
            data_structure,
        })
    }
    
    fn push_to_list(&self, key: &str, data: &str) -> Result<(), RedisSinkError> {
        let mut conn = self.connection.lock().unwrap();
        conn.rpush::<_, _, ()>(key, data).context(CommandSnafu)?;
        Ok(())
    }
    
    fn push_to_stream(&self, key: &str, data: &str) -> Result<(), RedisSinkError> {
        let mut conn = self.connection.lock().unwrap();
        conn.xadd::<_, _, _, _, ()>(key, "*", &[("data", data)]).context(CommandSnafu)?;
        Ok(())
    }
    
    fn publish(&self, channel: &str, data: &str) -> Result<(), RedisSinkError> {
        let mut conn = self.connection.lock().unwrap();
        conn.publish::<_, _, ()>(channel, data).context(CommandSnafu)?;
        Ok(())
    }
}

impl<In: Serialize> SinkExt<In> for RedisSink {
    fn build(id: &str) -> Self {
        let redis_url = dotenvy::var(format!("REDIS_URL_{}", id).as_str())
            .or_else(|_| dotenvy::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        
        let key_prefix = dotenvy::var(format!("REDIS_KEY_PREFIX_{}", id).as_str())
            .or_else(|_| dotenvy::var("REDIS_KEY_PREFIX"))
            .unwrap_or_else(|_| "cfvhub".to_string());
        
        let data_structure = dotenvy::var(format!("REDIS_DATA_STRUCTURE_{}", id).as_str())
            .or_else(|_| dotenvy::var("REDIS_DATA_STRUCTURE"))
            .unwrap_or_else(|_| "list".to_string());
        
        let data_structure = match data_structure.as_str() {
            "stream" => RedisDataStructure::Stream(format!("{}:stream", key_prefix)),
            "pubsub" => RedisDataStructure::PubSub(format!("{}:channel", key_prefix)),
            _ => RedisDataStructure::List(format!("{}:list", key_prefix)),
        };
        tracing::info!("RedisSink build: {}, {}, {:?}", redis_url, key_prefix, data_structure);
        Self::new(&redis_url, key_prefix, data_structure).unwrap()
    }
    
    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        let formated = formater.format(input);
        match formated {
            Ok(formated) => {
                let data = match formated {
                    Formated::String(s) => s,
                    Formated::Bytes(b) => {
                        match String::from_utf8(b) {
                            Ok(s) => s,
                            Err(e) => {
                                error!("Failed to convert bytes to string: {}", e);
                                return;
                            }
                        }
                    }
                };
                
                let result = match &self.data_structure {
                    RedisDataStructure::List(key) => self.push_to_list(key, &data),
                    RedisDataStructure::Stream(key) => self.push_to_stream(key, &data),
                    RedisDataStructure::PubSub(channel) => self.publish(channel, &data),
                };
                
                if let Err(e) = result {
                    error!("Redis sink error: {}", e);
                    // Try to reconnect on connection errors
                    if matches!(e, RedisSinkError::Connection { .. }) {
                        if let Ok(new_conn) = self.client.get_connection() {
                            *self.connection.lock().unwrap() = new_conn;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Format error: {}", e);
            }
        }
    }
}