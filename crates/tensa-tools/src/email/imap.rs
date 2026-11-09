//! IMAP-клиент (Контракт 6): папки, письма, поиск.
//!
//! Протокол отделён от транспорта: операции исполняются поверх любого
//! futures-потока (`AsyncRead + AsyncWrite + Unpin + Debug + Send`) —
//! продакшн оборачивает TCP в TLS (tokio-rustls + webpki-roots) и
//! приводит токиевский поток к futures через `tokio_util::compat`,
//! тесты используют локальный mock-сервер по обычному TCP (без сертификатов).
//!
//! MVP: ENVELOPE (от/кому/тема/дата) + флаги + BODY[TEXT]; разбор
//! BODYSTRUCTURE (вложения) и MIME-частей — следующая доводка.

use std::sync::Arc;

use async_imap::types::Fetch;
use futures::StreamExt;

use crate::email::connections::Account;
use crate::email::error::{shorten, EmailError};

/// Требования async-imap (runtime-tokio) к потоку сессии.
pub trait ImapStream:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send> ImapStream
    for T
{
}

/// Поток IMAP-сессии (TLS в проде, TCP в тестах).
pub type BoxStream = Box<dyn ImapStream>;

/// Сериализуемое письмо (Контракт 6: `Email`, упрощённо).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ImapEmail {
    /// UID письма.
    pub uid: u32,
    /// Отправитель (`Имя <user@host>` или `user@host`).
    pub from: String,
    /// Получатели (через запятую).
    pub to: String,
    /// Тема.
    pub subject: String,
    /// Дата из заголовка (строкой, как в ENVELOPE).
    pub date: String,
    /// Прочитано (флаг `\Seen`).
    pub read: bool,
    /// Текст письма (BODY[TEXT]).
    pub body: String,
}

/// Ошибка IMAP → EmailError с обрезкой текста.
fn imap_err<E: std::fmt::Display>(error: E) -> EmailError {
    EmailError::Imap(shorten(&error.to_string(), 300))
}

/// Войти в IMAP по готовому потоку (для TLS и mock-тестов).
async fn login_over(
    stream: BoxStream,
    account: &Account,
    credential: &str,
) -> Result<async_imap::Session<BoxStream>, EmailError> {
    let client = async_imap::Client::new(stream);
    let session = client
        .login(account.email.as_str(), credential)
        .await
        .map_err(|(error, _)| imap_err(error))?;
    Ok(session)
}

/// Папки аккаунта (без `\NoSelect`).
pub async fn list_folders_over(
    stream: BoxStream,
    account: &Account,
    credential: &str,
) -> Result<Vec<String>, EmailError> {
    let mut session = login_over(stream, account, credential).await?;
    let mut folders = Vec::new();
    {
        // Поток списка держит изменяемый заем сессии — зона видимости до logout.
        let mut names = session.list(Some(""), Some("*")).await.map_err(imap_err)?;
        while let Some(name) = names.next().await {
            let name = name.map_err(imap_err)?;
            let selective = name
                .attributes()
                .iter()
                .any(|attribute| matches!(attribute, async_imap::types::NameAttribute::NoSelect));
            if !selective {
                folders.push(name.name().to_string());
            }
        }
    }
    let _ = session.logout().await;
    Ok(folders)
}

/// Последние `limit` писем из папки (по убыванию порядковых номеров).
pub async fn fetch_emails_over(
    stream: BoxStream,
    account: &Account,
    credential: &str,
    folder: &str,
    limit: usize,
) -> Result<Vec<ImapEmail>, EmailError> {
    let mut session = login_over(stream, account, credential).await?;
    let mailbox = session.select(folder).await.map_err(imap_err)?;
    let exists = usize::try_from(mailbox.exists).unwrap_or(0);
    let mut emails = Vec::new();
    if exists > 0 {
        let start = exists.saturating_sub(limit.saturating_sub(1)).max(1);
        let mut stream = session
            .fetch(
                format!("{start}:{exists}"),
                "(UID FLAGS ENVELOPE BODY.PEEK[TEXT])",
            )
            .await
            .map_err(imap_err)?;
        while let Some(fetch) = stream.next().await {
            let fetch = fetch.map_err(imap_err)?;
            emails.push(fetch_to_email(&fetch));
        }
        // Порядок: свежие сверху.
        emails.reverse();
    }
    let _ = session.logout().await;
    Ok(emails)
}

/// Поиск писем по тексту (IMAP `SEARCH TEXT`) в INBOX.
pub async fn search_emails_over(
    stream: BoxStream,
    account: &Account,
    credential: &str,
    query: &str,
) -> Result<Vec<ImapEmail>, EmailError> {
    let mut session = login_over(stream, account, credential).await?;
    session.select("INBOX").await.map_err(imap_err)?;
    let escaped = query.replace('\\', "\\\\").replace('"', "\\\"");
    let found = session
        .search(format!("TEXT \"{escaped}\""))
        .await
        .map_err(imap_err)?;
    let mut seqs: Vec<u32> = found.into_iter().collect();
    seqs.sort_unstable();
    let mut emails = Vec::new();
    if !seqs.is_empty() {
        let set = seqs
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut stream = session
            .fetch(set, "(UID FLAGS ENVELOPE BODY.PEEK[TEXT])")
            .await
            .map_err(imap_err)?;
        while let Some(fetch) = stream.next().await {
            let fetch = fetch.map_err(imap_err)?;
            emails.push(fetch_to_email(&fetch));
        }
    }
    let _ = session.logout().await;
    Ok(emails)
}

/// TLS-поток к IMAP-серверу (rustls + корни webpki), приведённый к futures.
async fn tls_stream(host: &str, port: u16) -> Result<BoxStream, EmailError> {
    let tcp = tokio::net::TcpStream::connect((host, port)).await?;
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|error| EmailError::Imap(format!("TLS: {error}")))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let dns = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|error| EmailError::Imap(format!("сервер «{host}»: {error}")))?;
    let tls = connector.connect(dns, tcp).await?;
    Ok(Box::new(tls))
}

/// Папки аккаунта (продакшн: TLS).
pub async fn list_folders(account: &Account, credential: &str) -> Result<Vec<String>, EmailError> {
    let stream = tls_stream(&account.imap_host, account.imap_port).await?;
    list_folders_over(Box::new(stream), account, credential).await
}

/// Последние письма папки (продакшн: TLS).
pub async fn fetch_emails(
    account: &Account,
    credential: &str,
    folder: &str,
    limit: usize,
) -> Result<Vec<ImapEmail>, EmailError> {
    let stream = tls_stream(&account.imap_host, account.imap_port).await?;
    fetch_emails_over(Box::new(stream), account, credential, folder, limit).await
}

/// Поиск писем в INBOX (продакшн: TLS).
pub async fn search_emails(
    account: &Account,
    credential: &str,
    query: &str,
) -> Result<Vec<ImapEmail>, EmailError> {
    let stream = tls_stream(&account.imap_host, account.imap_port).await?;
    search_emails_over(Box::new(stream), account, credential, query).await
}

/// Байты Cow<[u8]> как UTF-8 строка (без паник на не-UTF8).
fn bytes_to_string(value: Option<std::borrow::Cow<'_, [u8]>>) -> String {
    value
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

/// Адрес ENVELOPE в формат `Имя <user@host>`.
fn format_address(address: &async_imap::imap_proto::types::Address<'_>) -> String {
    let mailbox = bytes_to_string(address.mailbox.clone());
    let host = bytes_to_string(address.host.clone());
    let name = bytes_to_string(address.name.clone());
    let email = format!("{mailbox}@{host}");
    if name.trim().is_empty() {
        email
    } else {
        format!("{name} <{email}>")
    }
}

/// Список адресов через запятую.
fn format_addresses(addresses: &Vec<async_imap::imap_proto::types::Address<'_>>) -> String {
    addresses
        .iter()
        .map(format_address)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Письмо из ответа FETCH.
fn fetch_to_email(fetch: &Fetch) -> ImapEmail {
    let envelope = fetch.envelope();
    let from = envelope
        .and_then(|envelope| envelope.from.as_ref())
        .map(format_addresses)
        .unwrap_or_default();
    let to = envelope
        .and_then(|envelope| envelope.to.as_ref())
        .map(format_addresses)
        .unwrap_or_default();
    let subject = bytes_to_string(envelope.and_then(|envelope| envelope.subject.clone()));
    let date = bytes_to_string(envelope.and_then(|envelope| envelope.date.clone()));
    let read = fetch
        .flags()
        .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
    let body = fetch
        .body()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default();
    ImapEmail {
        uid: fetch.uid.unwrap_or(0),
        from,
        to,
        subject,
        date,
        read,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    /// Адрес без имени — только email.
    #[test]
    fn format_address_without_name_is_plain_email() {
        let address = async_imap::imap_proto::types::Address {
            name: None,
            adl: None,
            mailbox: Some(Cow::Borrowed(b"ivan".as_slice())),
            host: Some(Cow::Borrowed(b"example.com".as_slice())),
        };
        assert_eq!(format_address(&address), "ivan@example.com");
    }

    /// Адрес с именем — `Имя <email>`.
    #[test]
    fn format_address_with_name_keeps_display_name() {
        let address = async_imap::imap_proto::types::Address {
            name: Some(Cow::Borrowed("Иван".as_bytes())),
            adl: None,
            mailbox: Some(Cow::Borrowed(b"ivan".as_slice())),
            host: Some(Cow::Borrowed(b"example.com".as_slice())),
        };
        assert_eq!(format_address(&address), "Иван <ivan@example.com>");
    }

    /// Mock-сервер IMAP: логин, SELECT, FETCH, LOGOUT по обычному TCP.
    /// Проверяет протокольную логику `*_over` без сети и TLS.
    #[tokio::test]
    async fn fetch_emails_over_mock_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("порт");
        let port = listener.local_addr().expect("адрес").port();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("соединение");
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let (reader, mut writer) = socket.into_split();
            let mut lines = BufReader::new(reader).lines();
            writer
                .write_all(b"* OK mock ready\r\n")
                .await
                .expect("приветствие");
            while let Ok(Some(line)) = lines.next_line().await {
                let tag = line.split(' ').next().unwrap_or("*").to_string();
                if line.contains(" LOGIN ") {
                    writer
                        .write_all(format!("{tag} OK logged in\r\n").as_bytes())
                        .await
                        .expect("ответ логина");
                } else if line.contains(" SELECT ") {
                    writer.write_all(b"* 3 EXISTS\r\n").await.expect("exists");
                    writer
                        .write_all(format!("{tag} OK [READ-WRITE] selected\r\n").as_bytes())
                        .await
                        .expect("ответ select");
                } else if line.contains(" FETCH ") {
                    // Два письма: первое непрочитанное без тела, второе
                    // прочитанное с телом-литералом (5 байт = "hello").
                    writer
                        .write_all(
                            b"* 1 FETCH (UID 11 FLAGS () ENVELOPE (\"Mon, 09 Nov 2026 10:00:00 +0300\" \"April estimate\" ((NIL NIL \"ivan\" \"example.com\")) ((NIL NIL \"ivan\" \"example.com\")) ((NIL NIL \"ivan\" \"example.com\")) ((NIL NIL \"chief\" \"example.com\")) NIL NIL NIL \"<1@example.com>\"))\r\n",
                        )
                        .await
                        .expect("fetch 1");
                    writer
                        .write_all(
                            b"* 2 FETCH (UID 12 FLAGS (\\Seen) ENVELOPE (\"Mon, 09 Nov 2026 11:00:00 +0300\" \"With body\" ((NIL NIL \"olga\" \"example.com\")) ((NIL NIL \"olga\" \"example.com\")) ((NIL NIL \"olga\" \"example.com\")) ((NIL NIL \"chief\" \"example.com\")) NIL NIL NIL \"<2@example.com>\") BODY[] {5}\r\nhello)\r\n",
                        )
                        .await
                        .expect("fetch 2");
                    writer
                        .write_all(format!("{tag} OK fetch done\r\n").as_bytes())
                        .await
                        .expect("конец fetch");
                } else if line.contains(" LOGOUT ") {
                    writer.write_all(b"* BYE\r\n").await.expect("bye");
                    writer
                        .write_all(format!("{tag} OK logout\r\n").as_bytes())
                        .await
                        .expect("ответ logout");
                    break;
                } else {
                    writer
                        .write_all(format!("{tag} OK\r\n").as_bytes())
                        .await
                        .expect("generic ok");
                }
            }
        });

        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("подключение к mock");
        let account = crate::email::connections::Account {
            id: "test".to_string(),
            email: "user@example.com".to_string(),
            label: "Тест".to_string(),
            imap_host: "127.0.0.1".to_string(),
            imap_port: port,
            smtp_host: "127.0.0.1".to_string(),
            smtp_port: 587,
            smtp_tls: crate::email::autodetect::SmtpTls::StartTls,
        };
        let emails = fetch_emails_over(Box::new(tcp), &account, "пароль-приложения", "testbox", 10)
            .await
            .expect("письма из mock");

        assert_eq!(emails.len(), 2);
        // Свежие сверху: второе письмо (последний seq) первым.
        assert_eq!(emails[0].uid, 12);
        assert_eq!(emails[0].subject, "With body");
        assert_eq!(emails[0].from, "olga@example.com");
        assert_eq!(emails[0].body, "hello");
        assert!(emails[0].read, "флаг \\Seen прочитан");
        assert_eq!(emails[1].uid, 11);
        assert_eq!(emails[1].subject, "April estimate");
        assert_eq!(emails[1].from, "ivan@example.com");
        assert!(!emails[1].read);

        server.await.expect("mock-сервер");
    }
}
