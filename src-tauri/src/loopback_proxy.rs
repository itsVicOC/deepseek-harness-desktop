use std::{io, net::Ipv4Addr};

use cookie::Cookie;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use url::Url;

use crate::error::DesktopError;

const SESSION_COOKIE: &str = "dsh_desktop_session";
const TOKEN_QUERY: &str = "dsh_token";
const MAX_HEADER_BYTES: usize = 64 * 1024;

pub struct LoopbackProxy {
    pub port: u16,
    task: JoinHandle<()>,
}

impl LoopbackProxy {
    pub async fn start(upstream_port: u16, token: String) -> Result<Self, DesktopError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let token = token.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve_connection(socket, upstream_port, &token).await {
                        tracing::debug!("loopback proxy connection closed: {error}");
                    }
                });
            }
        });
        Ok(Self { port, task })
    }
}

impl Drop for LoopbackProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_connection(
    mut client: TcpStream,
    upstream_port: u16,
    token: &str,
) -> io::Result<()> {
    let request = read_request_head(&mut client).await?;
    let parsed = parse_request(&request)?;

    if query_token(&parsed.path).as_deref() == Some(token) {
        let location = path_without_token(&parsed.path);
        let response = format!(
            "HTTP/1.1 303 See Other\r\nLocation: {location}\r\nSet-Cookie: {SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/\r\nCache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        client.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    if !has_session_cookie(&parsed.headers, token) {
        client
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nCache-Control: no-store\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 12\r\nConnection: close\r\n\r\nUnauthorized",
            )
            .await?;
        return Ok(());
    }

    let mut upstream = TcpStream::connect((Ipv4Addr::LOCALHOST, upstream_port)).await?;
    upstream
        .write_all(&sanitized_request(&request, &parsed)?)
        .await?;
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

struct ParsedRequest {
    method: String,
    path: String,
    version: u8,
    head_len: usize,
    headers: Vec<(String, Vec<u8>)>,
}

fn parse_request(input: &[u8]) -> io::Result<ParsedRequest> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut request = httparse::Request::new(&mut headers);
    let head_len = match request.parse(input).map_err(invalid_request)? {
        httparse::Status::Complete(length) => length,
        httparse::Status::Partial => return Err(invalid_request("partial request head")),
    };
    Ok(ParsedRequest {
        method: request
            .method
            .ok_or_else(|| invalid_request("missing method"))?
            .to_owned(),
        path: request
            .path
            .ok_or_else(|| invalid_request("missing path"))?
            .to_owned(),
        version: request
            .version
            .ok_or_else(|| invalid_request("missing HTTP version"))?,
        head_len,
        headers: request
            .headers
            .iter()
            .map(|header| (header.name.to_owned(), header.value.to_vec()))
            .collect(),
    })
}

async fn read_request_head(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(4096);
    loop {
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(bytes);
        }
        if bytes.len() >= MAX_HEADER_BYTES {
            return Err(invalid_request("request headers are too large"));
        }
        let read = stream.read_buf(&mut bytes).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed before request headers",
            ));
        }
    }
}

fn has_session_cookie(headers: &[(String, Vec<u8>)], token: &str) -> bool {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .filter_map(|(_, value)| std::str::from_utf8(value).ok())
        .flat_map(Cookie::split_parse)
        .filter_map(Result::ok)
        .any(|cookie| cookie.name() == SESSION_COOKIE && cookie.value() == token)
}

fn query_token(path: &str) -> Option<String> {
    let url = Url::parse(&format!("http://localhost{path}")).ok()?;
    url.query_pairs()
        .find(|(name, _)| name == TOKEN_QUERY)
        .map(|(_, value)| value.into_owned())
}

fn path_without_token(path: &str) -> String {
    let Ok(url) = Url::parse(&format!("http://localhost{path}")) else {
        return "/".into();
    };
    let query = url
        .query_pairs()
        .filter(|(name, _)| name != TOKEN_QUERY)
        .fold(
            url::form_urlencoded::Serializer::new(String::new()),
            |mut serializer, (name, value)| {
                serializer.append_pair(&name, &value);
                serializer
            },
        )
        .finish();
    if query.is_empty() {
        url.path().to_owned()
    } else {
        format!("{}?{query}", url.path())
    }
}

fn sanitized_request(input: &[u8], request: &ParsedRequest) -> io::Result<Vec<u8>> {
    let mut output = format!(
        "{} {} HTTP/1.{}\r\n",
        request.method, request.path, request.version
    )
    .into_bytes();
    for (name, value) in &request.headers {
        if name.eq_ignore_ascii_case("referer") && referer_contains_token(value) {
            continue;
        }
        if name.eq_ignore_ascii_case("cookie") {
            let value = std::str::from_utf8(value).map_err(invalid_request)?;
            let retained = Cookie::split_parse(value)
                .filter_map(Result::ok)
                .filter(|cookie| cookie.name() != SESSION_COOKIE)
                .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
                .collect::<Vec<_>>();
            if retained.is_empty() {
                continue;
            }
            output.extend_from_slice(b"Cookie: ");
            output.extend_from_slice(retained.join("; ").as_bytes());
            output.extend_from_slice(b"\r\n");
            continue;
        }
        output.extend_from_slice(name.as_bytes());
        output.extend_from_slice(b": ");
        output.extend_from_slice(value);
        output.extend_from_slice(b"\r\n");
    }
    output.extend_from_slice(b"\r\n");
    output.extend_from_slice(&input[request.head_len..]);
    Ok(output)
}

fn referer_contains_token(value: &[u8]) -> bool {
    std::str::from_utf8(value)
        .ok()
        .and_then(|value| Url::parse(value).ok())
        .is_some_and(|url| url.query_pairs().any(|(name, _)| name == TOKEN_QUERY))
}

fn invalid_request(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{has_session_cookie, path_without_token, query_token, sanitized_request};

    #[test]
    fn token_query_is_removed_from_redirect() {
        let path = "/?mode=desktop&dsh_token=secret";
        assert_eq!(query_token(path).as_deref(), Some("secret"));
        assert_eq!(path_without_token(path), "/?mode=desktop");
    }

    #[test]
    fn session_cookie_is_authenticated_and_not_forwarded() {
        let input = b"GET /api HTTP/1.1\r\nHost: 127.0.0.1:4100\r\nCookie: theme=dark; dsh_desktop_session=secret\r\n\r\n";
        let request = super::parse_request(input).expect("parse request");
        assert!(has_session_cookie(&request.headers, "secret"));
        let sanitized = String::from_utf8(sanitized_request(input, &request).expect("sanitize"))
            .expect("utf-8 request");
        assert!(sanitized.contains("Cookie: theme=dark"));
        assert!(!sanitized.contains("secret"));
    }

    #[test]
    fn token_bearing_referer_is_not_forwarded() {
        let input = b"GET /api HTTP/1.1\r\nReferer: http://127.0.0.1:4100/?dsh_token=secret\r\nCookie: dsh_desktop_session=secret\r\n\r\n";
        let request = super::parse_request(input).expect("parse request");
        let sanitized = String::from_utf8(sanitized_request(input, &request).expect("sanitize"))
            .expect("utf-8 request");
        assert!(!sanitized.contains("Referer:"));
    }
}
