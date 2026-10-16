//! Интеграционные тесты tensa-tools: реальная отправка magic packet WoL на
//! локальный UDP-сокет и выполнение инструмента через реестр.

use std::net::{Ipv4Addr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tensa_tools::wol::magic::{build_magic_packet, MacAddress, MAGIC_PACKET_LEN};
use tensa_tools::{Tool, ToolRegistry, WolTool};

/// MAC для тестов.
const MAC: &str = "AA:BB:CC:DD:EE:FF";

/// Ожидаемый magic packet: 6 байт 0xFF + MAC 16 раз (ТЗ tensa-tools).
fn expected_packet() -> Vec<u8> {
    let mut packet = vec![0xFFu8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }
    packet
}

/// Локальный UDP-приёмник с портом, выделенным ОС.
fn receiver() -> (UdpSocket, u16) {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind локального сокета");
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("таймаут чтения");
    let port = socket.local_addr().expect("адрес сокета").port();
    (socket, port)
}

/// Принять один датаграмм и вернуть его содержимое.
fn recv_datagram(socket: &UdpSocket) -> Vec<u8> {
    let mut buffer = [0u8; 512];
    let (len, _from) = socket.recv_from(&mut buffer).expect("magic packet получен");
    buffer[..len].to_vec()
}

#[tokio::test]
async fn execute_sends_exact_magic_packet() {
    let (receiver, port) = receiver();

    let result = WolTool::new()
        .execute(json!({ "mac": MAC, "broadcast": "127.0.0.1", "port": port }))
        .await
        .expect("execute");

    assert_eq!(result["sent"], json!(true));
    assert_eq!(result["mac"], json!("AA:BB:CC:DD:EE:FF"));
    assert_eq!(result["broadcast"], json!("127.0.0.1"));
    assert_eq!(result["port"], json!(port));
    assert_eq!(result["packet_bytes"], json!(MAGIC_PACKET_LEN));

    let received = recv_datagram(&receiver);
    assert_eq!(received.len(), MAGIC_PACKET_LEN);
    assert_eq!(received, expected_packet());
}

#[tokio::test]
async fn execute_accepts_mac_without_separators() {
    let (receiver, port) = receiver();

    WolTool::new()
        .execute(json!({ "mac": "aabbccddeeff", "broadcast": "127.0.0.1", "port": port }))
        .await
        .expect("execute");

    assert_eq!(recv_datagram(&receiver), expected_packet());
}

#[tokio::test]
async fn registry_executes_wol_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(WolTool::new()));

    assert_eq!(registry.list(), vec!["wol".to_string()]);
    let tool = registry.get("wol").expect("wol зарегистрирован");
    assert_eq!(tool.name(), "wol");
    assert!(tool.requires_approval());

    let (receiver, port) = receiver();
    tool.execute(json!({ "mac": MAC, "broadcast": "127.0.0.1", "port": port }))
        .await
        .expect("execute через реестр");

    assert_eq!(recv_datagram(&receiver), expected_packet());
}

#[test]
fn magic_packet_builder_matches_spec() {
    let mac = MacAddress::parse("AA:BB:CC:DD:EE:FF").expect("валидный MAC");
    assert_eq!(
        build_magic_packet(&mac).as_slice(),
        expected_packet().as_slice()
    );
}
