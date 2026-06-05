use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const HYPHEN: &str = "-";
const UNDERSCORE: &str = "_";
const SPECIAL: &str = "!$%&*+,.?@^~";
const BRACKETS: &str = "[]{}()<>";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordOptions {
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub hyphen: bool,
    pub underscore: bool,
    pub special: bool,
    pub brackets: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 16,
            uppercase: true,
            lowercase: true,
            digits: true,
            hyphen: false,
            underscore: false,
            special: true,
            brackets: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolAction {
    Enter { command: String },
    Copy { value: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub action: ToolAction,
}

pub fn tool_results(
    query: &str,
    password_options: &PasswordOptions,
    menu_alias: &str,
) -> Vec<ToolResult> {
    let trimmed = query.trim();
    if !menu_alias.trim().is_empty() && trimmed == menu_alias.trim() {
        return tools_entry_results();
    }

    let Some((keyword, rest)) = split_tool_query(trimmed) else {
        return Vec::new();
    };

    match keyword.to_ascii_lowercase().as_str() {
        "enc" => encode_results(rest),
        "dec" => decode_results(rest),
        "pwd" => password_results(rest, password_options),
        "time" => time_results(rest),
        _ => Vec::new(),
    }
}

pub fn generate_password(
    options: &PasswordOptions,
    override_length: Option<usize>,
) -> Result<String, String> {
    let length = sanitize_password_length(override_length.unwrap_or(options.length));
    let mut groups = Vec::new();
    if options.uppercase {
        groups.push(UPPERCASE);
    }
    if options.lowercase {
        groups.push(LOWERCASE);
    }
    if options.digits {
        groups.push(DIGITS);
    }
    if options.hyphen {
        groups.push(HYPHEN);
    }
    if options.underscore {
        groups.push(UNDERSCORE);
    }
    if options.special {
        groups.push(SPECIAL);
    }
    if options.brackets {
        groups.push(BRACKETS);
    }

    if groups.is_empty() {
        return Err("至少选择一种密码字符集".into());
    }

    let charset = groups.concat();
    let chars = charset.chars().collect::<Vec<_>>();
    let mut rng = rand::rng();
    let mut password = String::with_capacity(length);

    for _ in 0..length {
        let index = rng.random_range(0..chars.len());
        password.push(chars[index]);
    }

    Ok(password)
}

pub fn sanitize_password_options(mut options: PasswordOptions) -> PasswordOptions {
    options.length = sanitize_password_length(options.length);
    if !options.uppercase
        && !options.lowercase
        && !options.digits
        && !options.hyphen
        && !options.underscore
        && !options.special
        && !options.brackets
    {
        options.uppercase = true;
        options.lowercase = true;
        options.digits = true;
    }
    options
}

fn sanitize_password_length(length: usize) -> usize {
    length.clamp(4, 128)
}

fn tools_entry_results() -> Vec<ToolResult> {
    [
        (
            "enc",
            "编码工具 enc",
            "输入 enc 原始内容，返回 Unicode、URL、UTF-16、Base64、MD5、十六进制、SHA1 计算结果",
        ),
        (
            "dec",
            "解码工具 dec",
            "输入 dec 原始内容，返回 Unicode、URL、UTF-16、Base64、十六进制、HTML 实体和 URL 参数 JSON 解析结果",
        ),
        (
            "pwd",
            "随机密码 pwd",
            "输入 pwd 使用默认策略生成密码；输入 pwd 20 可临时生成 20 位密码，默认策略在设置 / 工具 中调整",
        ),
        (
            "time",
            "时间转换 time",
            "输入 time 时间或时间戳，自动识别秒、毫秒或日期时间，并返回本地时间、秒、毫秒和 ISO 时间",
        ),
    ]
    .into_iter()
    .map(|(command, title, subtitle)| ToolResult {
        id: format!("tool-entry:{command}"),
        title: title.into(),
        subtitle: subtitle.into(),
        action: ToolAction::Enter {
            command: format!("{command} "),
        },
    })
    .collect()
}

fn encode_results(input: &str) -> Vec<ToolResult> {
    if input.is_empty() {
        return tool_hint("enc", "输入 enc 原始内容后显示编码和摘要结果");
    }

    vec![
        copy_result("enc:unicode", "Unicode 编码", unicode_escape(input)),
        copy_result(
            "enc:url",
            "URL 编码",
            urlencoding::encode(input).into_owned(),
        ),
        copy_result("enc:utf16", "UTF-16 编码", utf16_escape(input)),
        copy_result(
            "enc:base64",
            "Base64 编码",
            BASE64_STANDARD.encode(input.as_bytes()),
        ),
        copy_result("enc:md5", "MD5 计算", md5_hex(input.as_bytes())),
        copy_result("enc:hex", "十六进制编码", hex_encode(input.as_bytes())),
        copy_result("enc:sha1", "SHA1 计算", sha1_hex(input.as_bytes())),
    ]
}

fn decode_results(input: &str) -> Vec<ToolResult> {
    if input.is_empty() {
        return tool_hint("dec", "输入 dec 原始内容后显示解码和解析结果");
    }

    let mut results = Vec::new();
    push_decode(
        &mut results,
        "dec:unicode",
        "Unicode 解码",
        unicode_unescape(input),
    );
    push_decode(
        &mut results,
        "dec:url",
        "URL 解码",
        urlencoding::decode(input)
            .map(|value| value.into_owned())
            .map_err(|error| error.to_string()),
    );
    push_decode(
        &mut results,
        "dec:utf16",
        "UTF-16 解码",
        utf16_unescape(input),
    );
    push_decode(
        &mut results,
        "dec:base64",
        "Base64 解码",
        BASE64_STANDARD
            .decode(input)
            .map_err(|error| error.to_string())
            .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string())),
    );
    push_decode(
        &mut results,
        "dec:hex",
        "十六进制解码",
        hex_decode(input)
            .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string())),
    );
    push_decode(
        &mut results,
        "dec:html",
        "HTML 实体解码",
        Ok(html_entity_decode(input)),
    );
    push_decode(
        &mut results,
        "dec:url-params",
        "URL 参数解析 JSON",
        parse_url_params_json(input),
    );
    results
}

fn password_results(input: &str, options: &PasswordOptions) -> Vec<ToolResult> {
    let length = input
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<usize>().ok());
    match generate_password(options, length) {
        Ok(password) => vec![copy_result(
            "pwd:generate",
            &format!("随机密码 {} 位", password.chars().count()),
            password,
        )],
        Err(error) => tool_hint("pwd", &error),
    }
}

fn time_results(input: &str) -> Vec<ToolResult> {
    if input.trim().is_empty() {
        return tool_hint("time", "输入 time 时间或时间戳后显示转换结果");
    }

    let Some(datetime) = parse_time_input(input.trim()) else {
        return tool_hint(
            "time",
            "无法识别时间；可输入 1717555200、1717555200000 或 2026-06-05 12:30:00",
        );
    };

    let local = datetime.with_timezone(&Local);
    let seconds = datetime.timestamp().to_string();
    let millis = datetime.timestamp_millis().to_string();
    vec![
        copy_result(
            "time:local",
            "本地时间",
            local.format("%Y-%m-%d %H:%M:%S %:z").to_string(),
        ),
        copy_result("time:seconds", "秒级时间戳", seconds),
        copy_result("time:millis", "毫秒级时间戳", millis),
        copy_result("time:iso", "ISO 时间", datetime.to_rfc3339()),
    ]
}

fn copy_result(id: &str, title: &str, value: String) -> ToolResult {
    ToolResult {
        id: format!("tool:{id}"),
        title: title.into(),
        subtitle: value.clone(),
        action: ToolAction::Copy { value },
    }
}

fn tool_hint(id: &str, subtitle: &str) -> Vec<ToolResult> {
    vec![ToolResult {
        id: format!("tool-hint:{id}"),
        title: "工具提示".into(),
        subtitle: subtitle.into(),
        action: ToolAction::Enter {
            command: format!("{id} "),
        },
    }]
}

fn push_decode(
    results: &mut Vec<ToolResult>,
    id: &str,
    title: &str,
    decoded: Result<String, String>,
) {
    if let Ok(value) = decoded {
        results.push(copy_result(id, title, value));
    }
}

fn split_tool_query(query: &str) -> Option<(&str, &str)> {
    let mut parts = query.splitn(2, char::is_whitespace);
    let keyword = parts.next()?;
    let rest = parts.next().unwrap_or("").trim();
    Some((keyword, rest))
}

fn unicode_escape(input: &str) -> String {
    input
        .chars()
        .map(|character| format!("\\u{{{:X}}}", character as u32))
        .collect::<Vec<_>>()
        .join("")
}

fn utf16_escape(input: &str) -> String {
    input
        .encode_utf16()
        .map(|unit| format!("\\u{unit:04X}"))
        .collect::<Vec<_>>()
        .join("")
}

fn unicode_unescape(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let chars = input.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\\' && chars.get(index + 1) == Some(&'u') {
            if chars.get(index + 2) == Some(&'{') {
                let Some(end) = chars[index + 3..].iter().position(|char| *char == '}') else {
                    return Err("Unicode 转义缺少 }".into());
                };
                let hex = chars[index + 3..index + 3 + end].iter().collect::<String>();
                let value =
                    u32::from_str_radix(&hex, 16).map_err(|_| "Unicode 转义无效".to_string())?;
                let character =
                    char::from_u32(value).ok_or_else(|| "Unicode 码点无效".to_string())?;
                output.push(character);
                index += end + 4;
                continue;
            }
            if index + 5 < chars.len() {
                let hex = chars[index + 2..index + 6].iter().collect::<String>();
                let unit =
                    u16::from_str_radix(&hex, 16).map_err(|_| "Unicode 转义无效".to_string())?;
                let mut units = vec![unit];
                index += 6;
                while index + 5 < chars.len()
                    && chars[index] == '\\'
                    && chars.get(index + 1) == Some(&'u')
                    && chars.get(index + 2) != Some(&'{')
                {
                    let hex = chars[index + 2..index + 6].iter().collect::<String>();
                    units.push(
                        u16::from_str_radix(&hex, 16)
                            .map_err(|_| "Unicode 转义无效".to_string())?,
                    );
                    index += 6;
                }
                let decoded = char::decode_utf16(units)
                    .map(|item| item.map_err(|_| "Unicode 代理对无效".to_string()))
                    .collect::<Result<String, String>>()?;
                output.push_str(&decoded);
                continue;
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    Ok(output)
}

fn utf16_unescape(input: &str) -> Result<String, String> {
    let mut units = Vec::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if !rest.starts_with("\\u") || rest.len() < 6 {
            return unicode_unescape(input);
        }
        let hex = &rest[2..6];
        units.push(u16::from_str_radix(hex, 16).map_err(|_| "UTF-16 转义无效".to_string())?);
        index += 6;
    }
    char::decode_utf16(units)
        .map(|item| item.map_err(|_| "UTF-16 代理对无效".to_string()))
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(input: &str) -> Result<Vec<u8>, String> {
    let normalized = input
        .trim()
        .strip_prefix("0x")
        .unwrap_or(input.trim())
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if normalized.len() % 2 != 0 {
        return Err("十六进制长度必须为偶数".into());
    }
    (0..normalized.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&normalized[index..index + 2], 16)
                .map_err(|_| "十六进制内容无效".into())
        })
        .collect()
}

fn html_entity_decode(input: &str) -> String {
    let mut output = input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    while let Some(start) = output.find("&#") {
        let Some(end_offset) = output[start..].find(';') else {
            break;
        };
        let end = start + end_offset;
        let entity = &output[start + 2..end];
        let value = if let Some(hex) = entity.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()
        } else {
            entity.parse::<u32>().ok()
        };
        let Some(character) = value.and_then(char::from_u32) else {
            break;
        };
        output.replace_range(start..=end, &character.to_string());
    }

    output
}

fn parse_url_params_json(input: &str) -> Result<String, String> {
    let query = input
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or(input);
    let query = query
        .split_once('#')
        .map(|(query, _)| query)
        .unwrap_or(query);
    if query.trim().is_empty() || !query.contains('=') {
        return Err("没有可解析的 URL 参数".into());
    }

    let mut object = Map::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = urlencoding::decode(key)
            .map_err(|error| error.to_string())?
            .into_owned();
        let value = urlencoding::decode(value)
            .map_err(|error| error.to_string())?
            .into_owned();
        match object.get_mut(&key) {
            Some(Value::Array(values)) => values.push(json!(value)),
            Some(existing) => {
                let first = existing.take();
                *existing = json!([first, value]);
            }
            None => {
                object.insert(key, json!(value));
            }
        }
    }

    serde_json::to_string_pretty(&Value::Object(object)).map_err(|error| error.to_string())
}

fn parse_time_input(input: &str) -> Option<DateTime<Utc>> {
    if input.chars().all(|character| character.is_ascii_digit()) {
        let value = input.parse::<i64>().ok()?;
        return if input.len() >= 13 {
            Utc.timestamp_millis_opt(value).single()
        } else {
            Utc.timestamp_opt(value, 0).single()
        };
    }

    if let Ok(datetime) = DateTime::parse_from_rfc3339(input) {
        return Some(datetime.with_timezone(&Utc));
    }

    for pattern in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(input, pattern) {
            return Local
                .from_local_datetime(&naive)
                .single()
                .map(|value| value.with_timezone(&Utc));
        }
    }

    None
}

fn md5_hex(input: &[u8]) -> String {
    let mut message = input.to_vec();
    let bit_len = (message.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0 = 0x67452301u32;
    let mut b0 = 0xefcdab89u32;
    let mut c0 = 0x98badcfeu32;
    let mut d0 = 0x10325476u32;
    let shifts = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let constants = (0..64)
        .map(|i| {
            ((f64::sin((i + 1) as f64).abs() * 4294967296.0).floor() as u64 & 0xffff_ffff) as u32
        })
        .collect::<Vec<_>>();

    for chunk in message.chunks(64) {
        let words = chunk
            .chunks(4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect::<Vec<_>>();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | (!b & d), i)
            } else if i < 32 {
                ((d & b) | (!d & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | !d), (7 * i) % 16)
            };
            let next = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(constants[i])
                    .wrapping_add(words[g])
                    .rotate_left(shifts[i]),
            );
            a = d;
            d = c;
            c = b;
            b = next;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    [a0, b0, c0, d0]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha1_hex(input: &[u8]) -> String {
    let mut message = input.to_vec();
    let bit_len = (message.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0 = 0x67452301u32;
    let mut h1 = 0xefcdab89u32;
    let mut h2 = 0x98badcfeu32;
    let mut h3 = 0x10325476u32;
    let mut h4 = 0xc3d2e1f0u32;

    for chunk in message.chunks(64) {
        let mut words = [0u32; 80];
        for (i, word) in words.iter_mut().enumerate().take(16) {
            let start = i * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for i in 16..80 {
            words[i] = (words[i - 3] ^ words[i - 8] ^ words[i - 14] ^ words[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for (i, word) in words.iter().enumerate() {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5a827999)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ed9eba1)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8f1bbcdc)
            } else {
                (b ^ c ^ d, 0xca62c1d6)
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    [h0, h1, h2, h3, h4]
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_known_values() {
        assert_eq!(md5_hex(b"hello"), "5d41402abc4b2a76b9719d911017c592");
        assert_eq!(
            sha1_hex(b"hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
    }

    #[test]
    fn utf16_handles_surrogate_pairs() {
        assert_eq!(utf16_escape("😀"), "\\uD83D\\uDE00");
        assert_eq!(utf16_unescape("\\uD83D\\uDE00").expect("decode"), "😀");
    }

    #[test]
    fn unicode_unescape_accepts_braced_and_utf16_forms() {
        assert_eq!(unicode_unescape("\\u{1F600}").expect("decode"), "😀");
        assert_eq!(unicode_unescape("\\u4F60\\u597D").expect("decode"), "你好");
        assert_eq!(unicode_unescape("\\uD83D\\uDE00").expect("decode"), "😀");
    }

    #[test]
    fn url_params_parse_to_json() {
        let json = parse_url_params_json("https://example.com?a=1&b=%E4%BD%A0").expect("parse");
        assert!(json.contains("\"a\": \"1\""));
        assert!(json.contains("\"b\": \"你\""));
    }
}
