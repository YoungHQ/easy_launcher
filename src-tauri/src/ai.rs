use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::storage::{AiAssistant, AiMessage, AiModelProfile};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAction {
    Translate,
    Summarize,
    Explain,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequest {
    pub action: AiAction,
    pub text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStreamEvent {
    pub request_id: String,
    pub kind: AiStreamEventKind,
    pub content: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatSendRequest {
    pub request_id: String,
    pub assistant_id: String,
    pub conversation_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatStarted {
    pub request_id: String,
    pub conversation_id: String,
    pub user_message: AiMessage,
    pub assistant_message: AiMessage,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatDeltaEvent {
    pub request_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub delta: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatDoneEvent {
    pub request_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub content: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatErrorEvent {
    pub request_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub error: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatCancelledEvent {
    pub request_id: String,
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequestMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AiRequestParams {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub stream: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AiChatCompletionResult {
    pub content: String,
    pub cancelled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiStreamEventKind {
    Delta,
    Done,
    Error,
    Cancelled,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatCompletionStreamResponse {
    choices: Vec<ChatStreamChoice>,
}

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelListItem>,
}

#[derive(Deserialize)]
struct ModelListItem {
    id: String,
}

pub fn merge_ai_params(assistant: &AiAssistant, profile: &AiModelProfile) -> AiRequestParams {
    AiRequestParams {
        temperature: assistant.temperature.or(profile.temperature),
        top_p: assistant.top_p.or(profile.top_p),
        max_tokens: assistant.max_tokens.or(profile.max_tokens),
        presence_penalty: assistant.presence_penalty.or(profile.presence_penalty),
        frequency_penalty: assistant.frequency_penalty.or(profile.frequency_penalty),
        stream: assistant.stream.unwrap_or(profile.stream),
    }
}

pub fn build_ai_request_messages(
    assistant: &AiAssistant,
    stored_messages: &[AiMessage],
    user_message: &str,
) -> Vec<AiRequestMessage> {
    let mut messages = Vec::new();
    if !assistant.system_prompt.trim().is_empty() {
        messages.push(AiRequestMessage {
            role: "system".into(),
            content: assistant.system_prompt.trim().into(),
        });
    }
    for message in stored_messages {
        if matches!(message.role.as_str(), "user" | "assistant" | "system")
            && message.status == "complete"
            && !message.content.trim().is_empty()
        {
            messages.push(AiRequestMessage {
                role: message.role.clone(),
                content: message.content.clone(),
            });
        }
    }
    messages.push(AiRequestMessage {
        role: "user".into(),
        content: user_message.trim().into(),
    });
    messages
}

pub fn validate_executable_profile(profile: &AiModelProfile) -> Result<(), String> {
    if profile.provider_type != "openai_compatible" {
        return Err("v1 仅支持 OpenAI 兼容接口模型配置".into());
    }
    if !profile.enabled {
        return Err("模型配置已禁用".into());
    }
    if profile.base_url.trim().is_empty() {
        return Err("请先配置模型 Base URL".into());
    }
    if profile.model_name.trim().is_empty() {
        return Err("请先配置模型名称".into());
    }
    Ok(())
}

pub async fn test_openai_compatible_profile(profile: AiModelProfile) -> Result<String, String> {
    validate_executable_profile(&profile)?;
    let params = AiRequestParams {
        temperature: None,
        top_p: None,
        max_tokens: Some(8),
        presence_penalty: None,
        frequency_penalty: None,
        stream: false,
    };
    let messages = vec![AiRequestMessage {
        role: "user".into(),
        content: "ping".into(),
    }];
    let content = send_openai_compatible_chat(
        &profile,
        &params,
        &messages,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .await?
    .content;
    Ok(if content.trim().is_empty() {
        "连接成功".into()
    } else {
        "连接成功，模型已响应".into()
    })
}

pub async fn list_openai_compatible_models(
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let base_url = base_url.trim();
    let api_key = api_key.trim();
    if base_url.is_empty() {
        return Err("请先配置模型 Base URL".into());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("创建 AI 客户端失败：{error}"))?;
    let mut request = client.get(models_endpoint(base_url));
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|error| redact_secret_text(&format!("AI 网络请求失败：{error}"), api_key))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| redact_secret_text(&format!("读取 AI 响应失败：{error}"), api_key))?;
    if !status.is_success() {
        return Err(format!(
            "模型列表获取失败（{status}）：{}",
            summarize_secret_error_body(&body, api_key)
        ));
    }

    let parsed: ModelListResponse =
        serde_json::from_str(&body).map_err(|error| format!("模型列表解析失败：{error}"))?;
    let mut models: Vec<String> = parsed
        .data
        .into_iter()
        .map(|model| model.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    models.sort_by_key(|id| id.to_lowercase());
    models.dedup();

    if models.is_empty() {
        return Err("模型列表为空".into());
    }

    Ok(models)
}

pub async fn send_openai_compatible_chat<F>(
    profile: &AiModelProfile,
    params: &AiRequestParams,
    messages: &[AiRequestMessage],
    cancelled: Arc<AtomicBool>,
    mut on_delta: F,
) -> Result<AiChatCompletionResult, String>
where
    F: FnMut(String) + Send + 'static,
{
    validate_executable_profile(profile)?;
    if messages.is_empty() {
        return Err("没有可发送的聊天消息".into());
    }

    let endpoint = chat_completions_endpoint(&profile.base_url);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| format!("创建 AI 客户端失败：{error}"))?;
    let mut body = serde_json::json!({
        "model": profile.model_name.trim(),
        "messages": messages,
        "stream": params.stream
    });
    append_optional_param(&mut body, "temperature", params.temperature);
    append_optional_param(&mut body, "top_p", params.top_p);
    append_optional_i64_param(&mut body, "max_tokens", params.max_tokens);
    append_optional_param(&mut body, "presence_penalty", params.presence_penalty);
    append_optional_param(&mut body, "frequency_penalty", params.frequency_penalty);

    let mut request = client.post(endpoint).json(&body);
    if !profile.api_key.trim().is_empty() {
        request = request.bearer_auth(profile.api_key.trim());
    }
    let response = request
        .send()
        .await
        .map_err(|error| redact_profile_secret(&format!("AI 网络请求失败：{error}"), profile))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.map_err(|error| {
            redact_profile_secret(&format!("读取 AI 响应失败：{error}"), profile)
        })?;
        return Err(format!(
            "AI 请求失败（{status}）：{}",
            summarize_profile_error_body(&body, profile)
        ));
    }

    if !params.stream {
        let body = response.text().await.map_err(|error| {
            redact_profile_secret(&format!("读取 AI 响应失败：{error}"), profile)
        })?;
        let content = extract_content(&body).ok_or_else(|| "AI 响应格式错误".to_string())?;
        return Ok(AiChatCompletionResult {
            content,
            cancelled: false,
        });
    }

    let mut response = response;
    let mut buffer = String::new();
    let mut output = String::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| redact_profile_secret(&format!("读取 AI 流失败：{error}"), profile))?
    {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(AiChatCompletionResult {
                content: output,
                cancelled: true,
            });
        }
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        let deltas = drain_stream_deltas(&mut buffer);
        for delta in deltas {
            output.push_str(&delta);
            on_delta(delta);
        }
    }

    if cancelled.load(Ordering::Relaxed) {
        return Ok(AiChatCompletionResult {
            content: output,
            cancelled: true,
        });
    }
    for delta in drain_stream_deltas(&mut buffer) {
        output.push_str(&delta);
        on_delta(delta);
    }

    Ok(AiChatCompletionResult {
        content: output,
        cancelled: false,
    })
}

#[derive(Deserialize)]
struct ChatStreamChoice {
    delta: ChatStreamDelta,
}

#[derive(Deserialize)]
struct ChatStreamDelta {
    content: Option<String>,
}

pub async fn run_ai_action(config: AiConfig, request: AiRequest) -> Result<String, String> {
    validate_config(&config)?;
    let text = request.text.trim();
    if text.is_empty() {
        return Err("没有可处理的文本".into());
    }

    let endpoint = chat_completions_endpoint(&config.base_url);
    let prompt = prompt_for_action(&request.action, text);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("创建 AI 客户端失败：{error}"))?;

    let mut request_builder = client.post(endpoint);
    if !config.api_key.trim().is_empty() {
        request_builder = request_builder.bearer_auth(config.api_key.trim());
    }

    let response = request_builder
        .json(&serde_json::json!({
            "model": config.model.trim(),
            "messages": [
                {
                    "role": "system",
                    "content": action_system_prompt(&config)
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "stream": false
        }))
        .send()
        .await
        .map_err(|error| redact_ai_secret(&format!("AI 网络请求失败：{error}"), &config))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| redact_ai_secret(&format!("读取 AI 响应失败：{error}"), &config))?;

    if !status.is_success() {
        return Err(format!(
            "AI 请求失败（{status}）：{}",
            summarize_error_body(&body, &config)
        ));
    }

    extract_content(&body).ok_or_else(|| "AI 响应格式错误".into())
}

pub async fn run_ai_action_stream<F>(
    request_id: String,
    config: AiConfig,
    request: AiRequest,
    cancelled: Arc<AtomicBool>,
    mut emit: F,
) where
    F: FnMut(AiStreamEvent) + Send + 'static,
{
    if let Err(error) = run_ai_action_stream_inner(
        request_id.clone(),
        config,
        request,
        cancelled.clone(),
        &mut emit,
    )
    .await
    {
        emit(AiStreamEvent {
            request_id,
            kind: if cancelled.load(Ordering::Relaxed) {
                AiStreamEventKind::Cancelled
            } else {
                AiStreamEventKind::Error
            },
            content: error,
        });
    }
}

async fn run_ai_action_stream_inner<F>(
    request_id: String,
    config: AiConfig,
    request: AiRequest,
    cancelled: Arc<AtomicBool>,
    emit: &mut F,
) -> Result<(), String>
where
    F: FnMut(AiStreamEvent),
{
    validate_config(&config)?;
    let text = request.text.trim();
    if text.is_empty() {
        return Err("没有可处理的文本".into());
    }

    let endpoint = chat_completions_endpoint(&config.base_url);
    let prompt = prompt_for_action(&request.action, text);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("创建 AI 客户端失败：{error}"))?;

    let mut request_builder = client.post(endpoint);
    if !config.api_key.trim().is_empty() {
        request_builder = request_builder.bearer_auth(config.api_key.trim());
    }

    let mut response = request_builder
        .json(&serde_json::json!({
            "model": config.model.trim(),
            "messages": [
                {
                    "role": "system",
                    "content": action_system_prompt(&config)
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "stream": true
        }))
        .send()
        .await
        .map_err(|error| redact_ai_secret(&format!("AI 网络请求失败：{error}"), &config))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .map_err(|error| redact_ai_secret(&format!("读取 AI 响应失败：{error}"), &config))?;
        return Err(format!(
            "AI 请求失败（{status}）：{}",
            summarize_error_body(&body, &config)
        ));
    }

    let mut buffer = String::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| redact_ai_secret(&format!("读取 AI 流失败：{error}"), &config))?
    {
        if cancelled.load(Ordering::Relaxed) {
            return Err("AI 请求已取消".into());
        }

        buffer.push_str(&String::from_utf8_lossy(&chunk));
        process_stream_buffer(&request_id, &mut buffer, emit);
    }

    if cancelled.load(Ordering::Relaxed) {
        return Err("AI 请求已取消".into());
    }

    process_stream_buffer(&request_id, &mut buffer, emit);
    emit(AiStreamEvent {
        request_id,
        kind: AiStreamEventKind::Done,
        content: String::new(),
    });

    Ok(())
}

fn validate_config(config: &AiConfig) -> Result<(), String> {
    if config.base_url.trim().is_empty() {
        return Err("请先配置 AI Base URL".into());
    }
    if config.model.trim().is_empty() {
        return Err("请先配置 AI Model".into());
    }

    Ok(())
}

fn action_system_prompt(config: &AiConfig) -> String {
    let prompt = config.system_prompt.trim();
    if prompt.is_empty() {
        "你是 Easy Launcher 的内置文本处理助手。请直接给出结果，不要输出无关解释。".into()
    } else {
        prompt.into()
    }
}

fn prompt_for_action(action: &AiAction, text: &str) -> String {
    match action {
        AiAction::Translate => format!("请将以下内容翻译为简体中文，保留原意：\n\n{text}"),
        AiAction::Summarize => format!("请用简体中文总结以下内容，控制在 5 条要点以内：\n\n{text}"),
        AiAction::Explain => format!("请用简体中文解释以下内容，说明关键概念和上下文：\n\n{text}"),
    }
}

fn chat_completions_endpoint(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

fn models_endpoint(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/models") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

fn extract_content(body: &str) -> Option<String> {
    let response: ChatCompletionResponse = serde_json::from_str(body).ok()?;
    response
        .choices
        .into_iter()
        .find_map(|choice| choice.message.content)
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
}

fn process_stream_buffer<F>(request_id: &str, buffer: &mut String, emit: &mut F)
where
    F: FnMut(AiStreamEvent),
{
    for content in drain_stream_deltas(buffer) {
        emit(AiStreamEvent {
            request_id: request_id.to_string(),
            kind: AiStreamEventKind::Delta,
            content,
        });
    }
}

fn drain_stream_deltas(buffer: &mut String) -> Vec<String> {
    let mut deltas = Vec::new();
    while let Some(newline_index) = buffer.find('\n') {
        let mut line = buffer[..newline_index].trim().to_string();
        buffer.replace_range(..=newline_index, "");

        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if let Some(data) = line.strip_prefix("data:") {
            line = data.trim().to_string();
        } else {
            continue;
        }

        if line == "[DONE]" {
            continue;
        }

        if let Some(content) = extract_stream_delta(&line) {
            deltas.push(content);
        }
    }
    deltas
}

fn extract_stream_delta(line: &str) -> Option<String> {
    let response: ChatCompletionStreamResponse = serde_json::from_str(line).ok()?;
    response
        .choices
        .into_iter()
        .find_map(|choice| choice.delta.content)
        .filter(|content| !content.is_empty())
}

fn summarize_error_body(body: &str, config: &AiConfig) -> String {
    let summary = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(200).collect());

    redact_ai_secret(&summary, config)
}

fn redact_ai_secret(text: &str, config: &AiConfig) -> String {
    let secret = config.api_key.trim();
    if secret.is_empty() {
        return text.to_string();
    }

    text.replace(secret, "[REDACTED]")
}

fn append_optional_param(body: &mut serde_json::Value, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        body[key] = serde_json::json!(value);
    }
}

fn append_optional_i64_param(body: &mut serde_json::Value, key: &str, value: Option<i64>) {
    if let Some(value) = value {
        body[key] = serde_json::json!(value);
    }
}

fn summarize_profile_error_body(body: &str, profile: &AiModelProfile) -> String {
    summarize_secret_error_body(body, profile.api_key.trim())
}

fn summarize_secret_error_body(body: &str, secret: &str) -> String {
    let summary = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(200).collect());

    redact_secret_text(&summary, secret)
}

pub fn redact_profile_secret(text: &str, profile: &AiModelProfile) -> String {
    redact_secret_text(text, profile.api_key.trim())
}

fn redact_secret_text(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        return text.to_string();
    }
    text.replace(secret, "[REDACTED]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Mutex;
    use std::thread::{self, JoinHandle};
    use std::time::Duration as StdDuration;

    struct MockChatServer {
        base_url: String,
        handle: JoinHandle<()>,
    }

    fn spawn_mock_chat_server<F>(handler: F) -> MockChatServer
    where
        F: FnOnce(String, TcpStream) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock chat server");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("mock server addr")
        );
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept mock request");
            let request = read_http_request(&mut stream);
            handler(request, stream);
        });

        MockChatServer { base_url, handle }
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(StdDuration::from_secs(2)))
            .expect("set mock read timeout");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let mut header_end = None;
        while header_end.is_none() {
            let read = stream.read(&mut buffer).expect("read mock headers");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
        }

        if let Some(header_end) = header_end {
            let headers = String::from_utf8_lossy(&request[..header_end + 4]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .or_else(|| line.strip_prefix("Content-Length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            let body_start = header_end + 4;
            while request.len().saturating_sub(body_start) < content_length {
                let read = stream.read(&mut buffer).expect("read mock body");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
        }

        String::from_utf8_lossy(&request).to_string()
    }

    fn write_json_response(stream: &mut TcpStream, body: &str) {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("write mock json response");
    }

    fn write_sse_headers(stream: &mut TcpStream) {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n"
        )
        .expect("write mock sse headers");
    }

    fn test_profile(base_url: String, api_key: &str) -> AiModelProfile {
        AiModelProfile {
            id: "profile".into(),
            provider_type: "openai_compatible".into(),
            name: "Local".into(),
            base_url,
            api_key: api_key.into(),
            model_name: "qwen".into(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            stream: true,
            enabled: true,
            sort_order: 0,
            last_used_at: None,
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    fn chat_messages() -> Vec<AiRequestMessage> {
        vec![AiRequestMessage {
            role: "user".into(),
            content: "ping".into(),
        }]
    }

    fn request_params(stream: bool) -> AiRequestParams {
        AiRequestParams {
            temperature: Some(0.3),
            top_p: None,
            max_tokens: Some(128),
            presence_penalty: None,
            frequency_penalty: None,
            stream,
        }
    }

    #[test]
    fn openai_compatible_chat_allows_empty_api_key() {
        let server = spawn_mock_chat_server(|request, mut stream| {
            assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
            assert!(!request
                .to_ascii_lowercase()
                .contains("authorization: bearer"));
            assert!(request.contains(r#""model":"qwen""#));
            assert!(request.contains(r#""stream":false"#));
            assert!(request.contains(r#""temperature":0.3"#));
            assert!(request.contains(r#""max_tokens":128"#));
            write_json_response(
                &mut stream,
                r#"{"choices":[{"message":{"content":" pong "}}]}"#,
            );
        });
        let profile = test_profile(server.base_url.clone(), "");

        let result = tauri::async_runtime::block_on(send_openai_compatible_chat(
            &profile,
            &request_params(false),
            &chat_messages(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        ))
        .expect("chat request succeeds");

        assert_eq!(result.content, "pong");
        assert!(!result.cancelled);
        server.handle.join().expect("mock server finished");
    }

    #[test]
    fn openai_compatible_chat_sends_bearer_auth_when_api_key_exists() {
        let server = spawn_mock_chat_server(|request, mut stream| {
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret-token"));
            write_json_response(
                &mut stream,
                r#"{"choices":[{"message":{"content":"authenticated"}}]}"#,
            );
        });
        let profile = test_profile(server.base_url.clone(), "secret-token");

        let result = tauri::async_runtime::block_on(send_openai_compatible_chat(
            &profile,
            &request_params(false),
            &chat_messages(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        ))
        .expect("chat request succeeds");

        assert_eq!(result.content, "authenticated");
        server.handle.join().expect("mock server finished");
    }

    #[test]
    fn openai_compatible_models_list_uses_models_endpoint_and_auth() {
        let server = spawn_mock_chat_server(|request, mut stream| {
            assert!(request.starts_with("GET /v1/models HTTP/1.1"));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret-token"));
            write_json_response(
                &mut stream,
                r#"{"data":[{"id":"zeta"},{"id":"alpha"},{"id":"alpha"}]}"#,
            );
        });

        let models = tauri::async_runtime::block_on(list_openai_compatible_models(
            &server.base_url,
            "secret-token",
        ))
        .expect("models request succeeds");

        assert_eq!(models, vec!["alpha".to_string(), "zeta".to_string()]);
        server.handle.join().expect("mock server finished");
    }

    #[test]
    fn openai_compatible_chat_streams_deltas() {
        let server = spawn_mock_chat_server(|request, mut stream| {
            assert!(request.contains(r#""stream":true"#));
            write_sse_headers(&mut stream);
            write!(
                stream,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"Hel\"}}}}]}}\n\n"
            )
            .expect("write first delta");
            write!(
                stream,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"lo\"}}}}]}}\n\n"
            )
            .expect("write second delta");
            write!(stream, "data: [DONE]\n\n").expect("write done");
        });
        let profile = test_profile(server.base_url.clone(), "");
        let deltas = Arc::new(Mutex::new(Vec::new()));
        let captured_deltas = deltas.clone();

        let result = tauri::async_runtime::block_on(send_openai_compatible_chat(
            &profile,
            &request_params(true),
            &chat_messages(),
            Arc::new(AtomicBool::new(false)),
            move |delta| {
                captured_deltas
                    .lock()
                    .expect("capture streamed delta")
                    .push(delta);
            },
        ))
        .expect("stream request succeeds");

        assert_eq!(result.content, "Hello");
        assert_eq!(
            *deltas.lock().expect("read streamed deltas"),
            vec!["Hel".to_string(), "lo".to_string()]
        );
        assert!(!result.cancelled);
        server.handle.join().expect("mock server finished");
    }

    #[test]
    fn openai_compatible_chat_cancel_stops_stream() {
        let server = spawn_mock_chat_server(|_, mut stream| {
            write_sse_headers(&mut stream);
            write!(
                stream,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"A\"}}}}]}}\n\n"
            )
            .expect("write first delta");
            stream.flush().expect("flush first delta");
            thread::sleep(StdDuration::from_millis(50));
            write!(
                stream,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"B\"}}}}]}}\n\n"
            )
            .expect("write second delta");
        });
        let profile = test_profile(server.base_url.clone(), "");
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel_from_delta = cancelled.clone();

        let result = tauri::async_runtime::block_on(send_openai_compatible_chat(
            &profile,
            &request_params(true),
            &chat_messages(),
            cancelled,
            move |_| cancel_from_delta.store(true, Ordering::Relaxed),
        ))
        .expect("stream request can be cancelled");

        assert_eq!(result.content, "A");
        assert!(result.cancelled);
        server.handle.join().expect("mock server finished");
    }

    #[test]
    fn assistant_parameters_override_profile_defaults_when_present() {
        let profile = AiModelProfile {
            id: "profile".into(),
            provider_type: "openai_compatible".into(),
            name: "Local".into(),
            base_url: "http://127.0.0.1:11434".into(),
            api_key: "".into(),
            model_name: "qwen".into(),
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            presence_penalty: Some(0.1),
            frequency_penalty: Some(0.2),
            stream: true,
            enabled: true,
            sort_order: 0,
            last_used_at: None,
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        let assistant = AiAssistant {
            id: "assistant".into(),
            name: "Assistant".into(),
            icon: "AI".into(),
            description: "".into(),
            model_profile_id: profile.id.clone(),
            system_prompt: "".into(),
            temperature: Some(0.2),
            top_p: None,
            max_tokens: Some(512),
            presence_penalty: None,
            frequency_penalty: Some(0.4),
            stream: Some(false),
            enabled: true,
            sort_order: 0,
            last_used_at: None,
            created_at: "now".into(),
            updated_at: "now".into(),
        };

        let params = merge_ai_params(&assistant, &profile);

        assert_eq!(params.temperature, Some(0.2));
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.max_tokens, Some(512));
        assert_eq!(params.presence_penalty, Some(0.1));
        assert_eq!(params.frequency_penalty, Some(0.4));
        assert!(!params.stream);
    }

    #[test]
    fn builds_chat_completions_endpoint() {
        assert_eq!(
            chat_completions_endpoint("https://api.openai.com"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_endpoint("https://api.example.com/v1"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_endpoint("https://api.example.com/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_content_from_chat_response() {
        let body = r#"{
          "choices": [
            { "message": { "content": " result " } }
          ]
        }"#;

        assert_eq!(extract_content(body).as_deref(), Some("result"));
    }

    #[test]
    fn validates_ai_config() {
        let config = AiConfig {
            base_url: "".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "".into(),
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validates_ai_config_allows_empty_api_key() {
        let config = AiConfig {
            base_url: "http://127.0.0.1:11434".into(),
            api_key: "".into(),
            model: "local-model".into(),
            system_prompt: "".into(),
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn extracts_stream_delta() {
        let line = r#"{"choices":[{"delta":{"content":"Hi"}}]}"#;
        assert_eq!(extract_stream_delta(line).as_deref(), Some("Hi"));
    }

    #[test]
    fn processes_sse_stream_buffer() {
        let mut buffer = "data: {\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\n\n".to_string();
        let mut output = String::new();

        process_stream_buffer("request-1", &mut buffer, &mut |event| {
            if matches!(event.kind, AiStreamEventKind::Delta) {
                output.push_str(&event.content);
            }
        });

        assert_eq!(output, "A");
        assert!(buffer.is_empty());
    }

    #[test]
    fn ai_error_summary_redacts_api_key_from_json_message() {
        let config = AiConfig {
            base_url: "https://api.example.com".into(),
            api_key: "secret-token".into(),
            model: "model".into(),
            system_prompt: "".into(),
        };
        let body = r#"{"error":{"message":"bad key secret-token"}}"#;

        let summary = summarize_error_body(body, &config);

        assert_eq!(summary, "bad key [REDACTED]");
        assert!(!summary.contains("secret-token"));
    }

    #[test]
    fn ai_error_summary_redacts_api_key_from_plain_body() {
        let config = AiConfig {
            base_url: "https://api.example.com".into(),
            api_key: "secret-token".into(),
            model: "model".into(),
            system_prompt: "".into(),
        };
        let body = "upstream echoed secret-token in a plain error";

        let summary = summarize_error_body(body, &config);

        assert_eq!(summary, "upstream echoed [REDACTED] in a plain error");
        assert!(!summary.contains("secret-token"));
    }
}
