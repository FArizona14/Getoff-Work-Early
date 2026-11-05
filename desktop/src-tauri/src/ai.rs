use crate::{reports::{ReportCandidate, ReportInput}, repository::Store};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
    pub has_api_key: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsInput {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub clear_api_key: bool,
}

pub struct AiRuntimeSettings {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
}

impl Store {
    pub fn ai_settings(&self) -> Result<AiSettings, String> {
        Ok(AiSettings {
            enabled: self.setting("ai_enabled").map(|value| value == "true").unwrap_or(false),
            endpoint: self.setting("ai_endpoint").unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".into()),
            model: self.setting("ai_model").unwrap_or_else(|| "gpt-4.1-mini".into()),
            has_api_key: self.secure_setting("ai_api_key")?.is_some(),
        })
    }

    pub fn save_ai_settings(&self, input: AiSettingsInput) -> Result<AiSettings, String> {
        let endpoint = input.endpoint.trim();
        let model = input.model.trim();
        if !endpoint.starts_with("https://") || endpoint.len() > 500 || endpoint.contains(char::is_whitespace) {
            return Err("AI 接口必须是有效的 HTTPS 地址".into());
        }
        if model.is_empty() || model.len() > 120 {
            return Err("AI 模型名称无效".into());
        }
        if let Some(key) = input.api_key.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            if key.len() > 500 { return Err("API Key 长度无效".into()); }
            self.set_secure_setting("ai_api_key", Some(key))?;
        } else if input.clear_api_key {
            self.set_secure_setting("ai_api_key", None)?;
        }
        let has_key = self.secure_setting("ai_api_key")?.is_some();
        if input.enabled && !has_key { return Err("启用 AI 前请填写 API Key".into()); }
        self.set("ai_enabled", &input.enabled.to_string())?;
        self.set("ai_endpoint", endpoint)?;
        self.set("ai_model", model)?;
        self.ai_settings()
    }

    pub fn ai_runtime_settings(&self) -> Result<AiRuntimeSettings, String> {
        let settings = self.ai_settings()?;
        if !settings.enabled { return Err("AI 整理尚未启用".into()); }
        Ok(AiRuntimeSettings {
            endpoint: settings.endpoint,
            model: settings.model,
            api_key: self.secure_setting("ai_api_key")?.ok_or("尚未配置 AI API Key")?,
        })
    }
}

fn prompt(input: &ReportInput, tasks: &[ReportCandidate]) -> Result<String, String> {
    let facts = tasks.iter().map(|task| json!({
        "id": task.id,
        "title": task.title,
        "result": task.result,
        "status": task.status,
        "project": task.project,
        "priority": task.priority,
        "plannedDate": task.planned_date,
        "completedAt": task.completed_at
    })).collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "reportType": input.report_type,
        "rangeStart": input.range_start,
        "rangeEnd": input.range_end,
        "template": input.template,
        "temporaryNote": input.note,
        "selectedRecords": facts
    })).map_err(|_| "AI 输入整理失败".into())
}

pub fn generate(settings: AiRuntimeSettings, input: &ReportInput, tasks: &[ReportCandidate]) -> Result<String, String> {
    let structure = if input.report_type == "daily" {
        "今日完成；关键成果 / 数据；进行中事项；风险与阻塞；明日计划"
    } else {
        "本周重点工作；主要成果与指标进展；问题、风险与协作需求；个人学习；下周计划"
    };
    let body = json!({
        "model": settings.model,
        "messages": [
            {"role":"system","content":format!("你是可信的个人工作报告整理助手。只使用用户明确提供的事实，禁止虚构指标、评价、完成比例或业务影响；缺少依据时写‘待补充’。输出 Markdown，严格包含以下五段：{structure}。每条内容应简洁、可核对，不要输出前言或解释。")},
            {"role":"user","content":prompt(input,tasks)?}
        ],
        "temperature": 0.2
    });
    let response = post_json(&settings.endpoint, &settings.api_key, &body.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&response).map_err(|_| "AI 服务返回了无法解析的响应".to_string())?;
    let content = value.pointer("/choices/0/message/content").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()).ok_or_else(|| {
        value.pointer("/error/message").and_then(|message|message.as_str()).map(|message|format!("AI 服务错误：{message}")).unwrap_or_else(||"AI 服务未返回报告正文".into())
    })?;
    if content.chars().count() > 100_000 { return Err("AI 返回内容过长，已停止保存".into()); }
    Ok(content.to_string())
}

#[cfg(windows)]
fn post_json(endpoint: &str, api_key: &str, body: &str) -> Result<String, String> {
    use std::{ffi::c_void, ptr::{null, null_mut}};
    type Internet = *mut c_void;
    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpOpen(agent: *const u16, access_type: u32, proxy: *const u16, bypass: *const u16, flags: u32) -> Internet;
        fn WinHttpConnect(session: Internet, server: *const u16, port: u16, reserved: u32) -> Internet;
        fn WinHttpOpenRequest(connect: Internet, verb: *const u16, object: *const u16, version: *const u16, referer: *const u16, accept_types: *const *const u16, flags: u32) -> Internet;
        fn WinHttpSetTimeouts(handle: Internet, resolve: i32, connect: i32, send: i32, receive: i32) -> i32;
        fn WinHttpSendRequest(request: Internet, headers: *const u16, headers_length: u32, optional: *mut c_void, optional_length: u32, total_length: u32, context: usize) -> i32;
        fn WinHttpReceiveResponse(request: Internet, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(request: Internet, info_level: u32, name: *const u16, buffer: *mut c_void, buffer_length: *mut u32, index: *mut u32) -> i32;
        fn WinHttpQueryDataAvailable(request: Internet, available: *mut u32) -> i32;
        fn WinHttpReadData(request: Internet, buffer: *mut c_void, bytes_to_read: u32, bytes_read: *mut u32) -> i32;
        fn WinHttpCloseHandle(handle: Internet) -> i32;
    }
    struct Handle(Internet);
    impl Drop for Handle { fn drop(&mut self) { unsafe { WinHttpCloseHandle(self.0); } } }
    fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain(Some(0)).collect() }
    fn last_error(context: &str) -> String { format!("{context}失败（Windows 错误 {}）", unsafe { windows_sys::Win32::Foundation::GetLastError() }) }

    let url = endpoint.strip_prefix("https://").ok_or("AI 接口必须使用 HTTPS")?;
    let (authority, path) = url.split_once('/').map(|(host,path)|(host,format!("/{path}"))).unwrap_or((url,"/".into()));
    let (host, port) = authority.rsplit_once(':').and_then(|(host,port)|port.parse::<u16>().ok().map(|port|(host,port))).unwrap_or((authority,443));
    if host.is_empty() || path.len() > 2000 { return Err("AI 接口地址无效".into()); }
    let agent=wide("EarlyOffWork/0.3");let host=wide(host);let path=wide(&path);let verb=wide("POST");
    let session=Handle(unsafe{WinHttpOpen(agent.as_ptr(),4,null(),null(),0)});if session.0.is_null(){return Err(last_error("AI 网络会话创建"))}
    unsafe{WinHttpSetTimeouts(session.0,10_000,10_000,30_000,60_000)};
    let connect=Handle(unsafe{WinHttpConnect(session.0,host.as_ptr(),port,0)});if connect.0.is_null(){return Err(last_error("AI 服务连接"))}
    let request=Handle(unsafe{WinHttpOpenRequest(connect.0,verb.as_ptr(),path.as_ptr(),null(),null(),null(),0x0080_0000)});if request.0.is_null(){return Err(last_error("AI 请求创建"))}
    let headers=wide(&format!("Content-Type: application/json\r\nAuthorization: Bearer {api_key}\r\n"));let bytes=body.as_bytes();
    if unsafe{WinHttpSendRequest(request.0,headers.as_ptr(),u32::MAX,bytes.as_ptr() as *mut c_void,bytes.len() as u32,bytes.len() as u32,0)}==0{return Err(last_error("AI 请求发送"))}
    if unsafe{WinHttpReceiveResponse(request.0,null_mut())}==0{return Err(last_error("AI 响应接收"))}
    let mut status=0u32;let mut status_size=std::mem::size_of::<u32>() as u32;
    if unsafe{WinHttpQueryHeaders(request.0,19|0x2000_0000,null(),&mut status as *mut _ as *mut c_void,&mut status_size,null_mut())}==0{return Err(last_error("AI 状态读取"))}
    let mut output=Vec::new();
    loop { let mut available=0u32;if unsafe{WinHttpQueryDataAvailable(request.0,&mut available)}==0{return Err(last_error("AI 响应读取"))}if available==0{break}if output.len()+available as usize>2*1024*1024{return Err("AI 响应超过 2 MB，已停止读取".into())}let start=output.len();output.resize(start+available as usize,0);let mut read=0u32;if unsafe{WinHttpReadData(request.0,output[start..].as_mut_ptr() as *mut c_void,available,&mut read)}==0{return Err(last_error("AI 响应读取"))}output.truncate(start+read as usize); }
    let text=String::from_utf8(output).map_err(|_|"AI 响应编码无效")?;
    if !(200..300).contains(&status){return Err(serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|value|value.pointer("/error/message").and_then(|message|message.as_str()).map(ToOwned::to_owned)).map(|message|format!("AI 服务错误（{status}）：{message}")).unwrap_or_else(||format!("AI 服务返回 HTTP {status}")))}
    Ok(text)
}

#[cfg(not(windows))]
fn post_json(_: &str, _: &str, _: &str) -> Result<String, String> {
    Err("当前构建不支持 Windows AI 网络适配器".into())
}
