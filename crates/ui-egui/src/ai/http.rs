//! The assistant's HTTPS transport: one blocking request per call, on the worker thread.
//!
//! TLS is rustls with the operating system's certificate check, as Help ▸ Updates uses. The
//! address was checked by `pdfcraft_ai` (HTTPS, or plain HTTP to this computer only). Redirects are
//! not followed, so the key in a header never goes to another host. Proxies come from the
//! `HTTPS_PROXY` / `ALL_PROXY` environment variables. Nothing here is logged: headers carry the key.

use std::time::Duration;

use pdfcraft_ai::{AiError, HttpRequest, HttpResponse, MAX_REPLY_BYTES, Transport};

/// Sends each request with a new agent: requests are rare and minutes apart.
pub(crate) struct HttpTransport;

fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .proxy(ureq::Proxy::try_from_env())
        .timeout_connect(Some(Duration::from_secs(20)))
        // A long document and a slow model can take minutes; this stops a request that never ends.
        .timeout_global(Some(Duration::from_secs(300)))
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .user_agent(concat!("AJSPdfEditor/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent()
}

impl Transport for HttpTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, AiError> {
        let agent = agent();
        let net = |e: ureq::Error| AiError::Network(e.to_string());
        let response = match (request.method, &request.body) {
            ("POST", Some(body)) => {
                let mut r = agent.post(&request.url);
                for (name, value) in &request.headers {
                    r = r.header(name, value);
                }
                r.send(body.as_str()).map_err(net)?
            }
            _ => {
                let mut r = agent.get(&request.url);
                for (name, value) in &request.headers {
                    r = r.header(name, value);
                }
                r.call().map_err(net)?
            }
        };
        let status = response.status().as_u16();
        let mut response = response;
        let body = response.body_mut().with_config().limit(MAX_REPLY_BYTES).read_to_string().map_err(net)?;
        Ok(HttpResponse { status, body })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// One HTTP exchange on loopback: what arrived, and the answer given.
    fn serve_once(status: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut body_in = vec![0; length];
            reader.read_exact(&mut body_in).unwrap();
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            head + &String::from_utf8(body_in).unwrap()
        });
        (url, handle)
    }

    #[test]
    fn a_request_reaches_a_local_server_with_its_headers_and_body() {
        let (url, server) = serve_once("200 OK", r#"{"ok":true}"#);
        let request = HttpRequest {
            method: "POST",
            url: format!("{url}/v1/chat/completions"),
            headers: vec![("Authorization".into(), "Bearer sk-local".into()), ("Content-Type".into(), "application/json".into())],
            body: Some(r#"{"model":"llama3.2"}"#.into()),
        };
        let response = HttpTransport.send(&request).unwrap();
        assert_eq!(response, HttpResponse { status: 200, body: r#"{"ok":true}"#.into() });
        let seen = server.join().unwrap().to_ascii_lowercase();
        assert!(seen.starts_with("post /v1/chat/completions http/1.1"), "{seen}");
        assert!(seen.contains("authorization: bearer sk-local"));
        assert!(seen.contains("user-agent: ajspdfeditor/"));
        assert!(seen.ends_with(r#"{"model":"llama3.2"}"#));
    }

    #[test]
    fn error_statuses_come_back_as_responses_and_redirects_are_not_followed() {
        let (url, server) = serve_once("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#);
        let get = HttpRequest { method: "GET", url: format!("{url}/v1/models"), headers: vec![], body: None };
        assert_eq!(HttpTransport.send(&get).unwrap().status, 401);
        server.join().unwrap();
        let (url, server) = serve_once("302 Found\r\nLocation: http://127.0.0.1:9/elsewhere", "");
        let get = HttpRequest { method: "GET", url: format!("{url}/v1/models"), headers: vec![], body: None };
        assert_eq!(HttpTransport.send(&get).unwrap().status, 302);
        server.join().unwrap();
    }

    #[test]
    fn nobody_listening_is_a_network_error() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let get = HttpRequest { method: "GET", url: format!("http://127.0.0.1:{port}/v1/models"), headers: vec![], body: None };
        assert!(matches!(HttpTransport.send(&get), Err(AiError::Network(_))));
    }
}
