//! 认证工具模块
//!
//! 提供密码哈希（argon2id）、Session token 生成、
//! 端点 API Key 生成和哈希功能。

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::AppError;

const API_KEY_PREFIX: &str = "olr_";

/// 生成 argon2id 密码哈希
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Message(format!("密码哈希失败: {e}")))?
        .to_string();
    Ok(hash)
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| AppError::Message(format!("密码解析失败: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// 生成 64 字符 hex session token
pub fn generate_session_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}

/// 生成端点 API Key，格式: olr_{32字节 base64url}
pub fn generate_api_key() -> (String, String) {
    let random_bytes: [u8; 32] = rand::random();
    let raw = URL_SAFE_NO_PAD.encode(random_bytes);
    let full_key = format!("{API_KEY_PREFIX}{raw}");
    let prefix = full_key[..(API_KEY_PREFIX.len() + 12)].to_string();
    (full_key, prefix)
}

/// 生成随机 12 位密码
pub fn generate_password() -> String {
    use rand::Rng;
    let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..12)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

/// 计算 API Key 的 SHA-256 哈希（用于存储）
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// 计算 SHA-256 哈希
pub fn sha256_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}
