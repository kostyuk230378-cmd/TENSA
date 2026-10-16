//! Wake-on-LAN (ТЗ tensa-tools, точка входа WoL): пробуждение устройства
//! magic packet по UDP.
//!
//! Инструмент живёт модулем внутри крейта: ТЗ предполагает отдельный подкрейт
//! `crates/tensa-tools/wol`, выделение подкрейтов — отдельная задача (иначе
//! пришлось бы менять корневой `Cargo.toml` воркспейса).
//!
//! Сетевые действия выполняются инструментом только после решения владельца:
//! проверку ведёт `ApprovalStore` централизованно, поэтому [`WolTool`] лишь
//! объявляет `requires_approval() == true` и не обращается к хранилищу сам.

pub mod error;
pub mod magic;

use std::net::{Ipv4Addr, UdpSocket};

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::ToolsError;
use crate::Tool;

pub use error::WolError;
pub use magic::{
    build_magic_packet, MacAddress, MAC_LEN, MAC_REPEATS, MAGIC_PACKET_LEN, MAGIC_PREFIX_LEN,
};

/// Порт по умолчанию — стандартный порт Wake-on-LAN.
pub const DEFAULT_PORT: u16 = 9;

/// Адрес отправки по умолчанию — широковещательный `255.255.255.255`.
pub const DEFAULT_BROADCAST: Ipv4Addr = Ipv4Addr::BROADCAST;

/// Описание инструмента для LLM и интерфейса.
pub const DESCRIPTION: &str = "Wake-on-LAN: отправляет magic packet (6 байт 0xFF + MAC x16) \
по UDP на broadcast-адрес, чтобы включить устройство. Параметры: mac (обязательный, \
\"AA:BB:CC:DD:EE:FF\" или \"AABBCCDDEEFF\"), broadcast (IPv4, по умолчанию 255.255.255.255), \
port (по умолчанию 9).";

/// Разобранные параметры вызова `wol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WolParams {
    /// MAC-адрес пробуждаемого устройства.
    pub mac: MacAddress,
    /// Адрес отправки magic packet.
    pub broadcast: Ipv4Addr,
    /// UDP-порт отправки.
    pub port: u16,
}

impl WolParams {
    /// Разобрать параметры `execute`: `{"mac": "...", "broadcast": "...", "port": 9}`.
    ///
    /// Обязателен только `mac`; `broadcast` (IPv4) и `port` по умолчанию
    /// `255.255.255.255` и `9`. Явный `null` равнозначен отсутствию параметра,
    /// лишние ключи игнорируются.
    pub fn from_json(params: &Value) -> Result<Self, WolError> {
        let object = params
            .as_object()
            .ok_or(WolError::InvalidParams(json_type(params)))?;

        let mac_value = object.get("mac").ok_or(WolError::MissingParameter("mac"))?;
        let mac_raw = mac_value
            .as_str()
            .ok_or_else(|| WolError::InvalidParameter {
                name: "mac",
                expected: "string, \"AA:BB:CC:DD:EE:FF\" or \"AABBCCDDEEFF\"",
                actual: json_type(mac_value).to_string(),
            })?;
        let mac = MacAddress::parse(mac_raw)?;

        let broadcast = match object.get("broadcast") {
            None | Some(Value::Null) => DEFAULT_BROADCAST,
            Some(value) => {
                let raw = value.as_str().ok_or_else(|| WolError::InvalidParameter {
                    name: "broadcast",
                    expected: "string with IPv4 address, e.g. \"192.0.2.255\"",
                    actual: json_type(value).to_string(),
                })?;
                raw.parse::<Ipv4Addr>()
                    .map_err(|_| WolError::InvalidBroadcast(raw.to_string()))?
            }
        };

        let port = match object.get("port") {
            None | Some(Value::Null) => DEFAULT_PORT,
            Some(value) => {
                let number = value.as_u64().ok_or_else(|| WolError::InvalidParameter {
                    name: "port",
                    expected: "integer 1..=65535",
                    actual: json_type(value).to_string(),
                })?;
                u16::try_from(number)
                    .ok()
                    .filter(|port| *port != 0)
                    .ok_or_else(|| WolError::InvalidParameter {
                        name: "port",
                        expected: "integer 1..=65535",
                        actual: number.to_string(),
                    })?
            }
        };

        Ok(Self {
            mac,
            broadcast,
            port,
        })
    }
}

/// Отправить magic packet на адрес из параметров.
///
/// Возвращает число отправленных байт (длина magic packet — 102).
pub fn send_magic_packet(params: &WolParams) -> Result<usize, WolError> {
    let packet = build_magic_packet(&params.mac);
    tracing::debug!(
        mac = %params.mac,
        broadcast = %params.broadcast,
        port = params.port,
        packet_bytes = packet.len(),
        "wol: отправка magic packet"
    );

    // Исходящий сокет: эфемерный порт, broadcast разрешён явно.
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    let sent = socket.send_to(&packet, (params.broadcast, params.port))?;

    tracing::debug!(sent_bytes = sent, "wol: magic packet отправлен");
    Ok(sent)
}

/// Инструмент `wol`: пробуждение устройства по сети (Wake-on-LAN).
#[derive(Debug, Default, Clone, Copy)]
pub struct WolTool;

impl WolTool {
    /// Создать инструмент.
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WolTool {
    /// Имя инструмента в реестре.
    fn name(&self) -> &str {
        "wol"
    }

    /// Описание для LLM и интерфейса.
    fn description(&self) -> &str {
        DESCRIPTION
    }

    /// Разобрать параметры, отправить magic packet и вернуть отчёт об отправке.
    async fn execute(&self, params: Value) -> Result<Value, ToolsError> {
        let parsed = WolParams::from_json(&params)?;
        let sent = send_magic_packet(&parsed)?;

        Ok(json!({
            "sent": true,
            "mac": parsed.mac.to_string(),
            "broadcast": parsed.broadcast.to_string(),
            "port": parsed.port,
            "packet_bytes": sent,
        }))
    }

    /// Отправка пакета в сеть требует подтверждения владельца.
    fn requires_approval(&self) -> bool {
        true
    }
}

/// Имя типа JSON-значения — для сообщений об ошибке параметров.
fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
