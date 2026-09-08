//! Native desktop boundary. Webviews never receive the daemon credential.
use polymas_core::{
    error::{Result, require},
    secrets::Secret,
};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct BridgeError {
    pub code: String,
    pub message: String,
    pub status: u16,
}
impl BridgeError {
    fn new(code: &str, message: &str, status: u16) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            status,
        }
    }
}
pub struct NativeConnection {
    client: reqwest::Client,
    port: u16,
    token: Secret,
}
impl NativeConnection {
    pub fn new(port: u16, token: Secret) -> Result<Self> {
        require(
            port > 0 && (32..=256).contains(&token.expose().len()),
            "Некорректное подключение службы",
        )?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|_| {
                polymas_core::error::Error::Runtime(
                    "Не удалось создать локальное подключение".into(),
                )
            })?;
        Ok(Self {
            client,
            port,
            token,
        })
    }
    pub async fn request(
        &self,
        path: &str,
        body: Option<Value>,
    ) -> std::result::Result<Value, BridgeError> {
        let pathname = path.split('?').next().unwrap_or_default();
        if path.len() > 4096
            || !pathname.starts_with("/api/")
            || pathname.split('/').skip(1).any(|segment| {
                segment.is_empty()
                    || !segment
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            })
            || path
                .chars()
                .any(|c| c.is_control() || c == '#' || c == '\\')
        {
            return Err(BridgeError::new(
                "validation",
                "Разрешены только команды локального API",
                400,
            ));
        }
        let url = format!("http://127.0.0.1:{}{path}", self.port);
        let mut builder = if let Some(body) = body {
            let bytes = serde_json::to_vec(&body)
                .map_err(|_| BridgeError::new("validation", "Некорректное тело команды", 400))?;
            if bytes.len() > 1024 * 1024 {
                return Err(BridgeError::new(
                    "validation",
                    "Команда превышает 1 МиБ",
                    400,
                ));
            }
            self.client
                .post(&url)
                .header("content-type", "application/json")
                .body(bytes)
        } else {
            self.client.get(&url)
        };
        let mut authorization =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.token.expose()))
                .map_err(|_| BridgeError::new("authorization", "Некорректное подключение", 401))?;
        authorization.set_sensitive(true);
        builder = builder.header(reqwest::header::AUTHORIZATION, authorization);
        let network_error = |_: reqwest::Error| {
            BridgeError::new(
                "disconnected",
                "Служба не отвечает. Проверьте подключение и состояние задачи перед повтором команды.",
                503,
            )
        };
        let mut response = builder.send().await.map_err(network_error)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(BridgeError::new(
                "invalid_response",
                "Служба попыталась перенаправить локальную команду",
                502,
            ));
        }
        const LIMIT: usize = 16 * 1024 * 1024;
        if response.content_length().is_some_and(|n| n > LIMIT as u64) {
            return Err(BridgeError::new(
                "response_too_large",
                "Ответ превышает 16 МиБ; уменьшите страницу запроса",
                502,
            ));
        }
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if bytes.len() + chunk.len() > LIMIT {
                return Err(BridgeError::new(
                    "response_too_large",
                    "Ответ превышает 16 МиБ; уменьшите страницу запроса",
                    502,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| {
                BridgeError::new("invalid_response", "Служба вернула некорректный UTF-8", 502)
            })?
            .replace(self.token.expose(), "[REDACTED]");
        let value: Value = serde_json::from_str(&text).map_err(|_| {
            BridgeError::new("invalid_response", "Служба вернула некорректный JSON", 502)
        })?;
        if status.is_success() {
            Ok(value)
        } else {
            let code = value["code"].as_str().unwrap_or("request_failed");
            let message = value["message"].as_str().unwrap_or("Команда не выполнена");
            Err(BridgeError {
                code: code.chars().take(100).collect(),
                message: polymas_core::process::redact(
                    &message.chars().take(8192).collect::<String>(),
                ),
                status: status.as_u16(),
            })
        }
    }
}
