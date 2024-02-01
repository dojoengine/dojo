use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

use reqwest::Url;

/// Error thrown while parsing a socket address.
#[derive(thiserror::Error, Debug)]
pub enum SocketAddressParsingError {
    /// Failed to convert the string into a socket addr
    #[error("could not parse socket address: {0}")]
    Io(#[from] std::io::Error),
    /// Input must not be empty
    #[error("cannot parse socket address from empty string")]
    Empty,
    /// Failed to parse the address
    #[error("could not parse socket address from {0}")]
    Parse(String),
    /// Failed to parse port
    #[error("could not parse port: {0}")]
    Port(#[from] std::num::ParseIntError),
}

/// Parse a [SocketAddr] from a `str`.
///
/// The following formats are checked:
///
/// - If the value can be parsed as a `u16` or starts with `:` it is considered a port, and the
/// hostname is set to `localhost`.
/// - If the value contains `:` it is assumed to be the format `<host>:<port>`
/// - Otherwise it is assumed to be a hostname
///
/// An error is returned if the value is empty.
pub fn parse_socket_address(value: &str) -> anyhow::Result<SocketAddr, SocketAddressParsingError> {
    if value.is_empty() {
        return Err(SocketAddressParsingError::Empty);
    }

    if let Some(port) = value.strip_prefix(':').or_else(|| value.strip_prefix("localhost:")) {
        let port: u16 = port.parse()?;
        return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    if let Ok(port) = value.parse::<u16>() {
        return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    value
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| SocketAddressParsingError::Parse(value.to_string()))
}

/// Error thrown while parsing a URL.
#[derive(thiserror::Error, Debug)]
pub enum URLParsingError {
    #[error("cannot parse URL from empty string")]
    Empty,
    #[error("could not parse URL: {0}")]
    Parse(String),
    #[error("invalid scheme in URL: {0}")]
    InvalidScheme(String),
}

pub fn parse_url(value: &str) -> anyhow::Result<Url, URLParsingError> {
    if value.is_empty() {
        return Err(URLParsingError::Empty);
    }

    // Check if the value starts with "localhost:"
    if value.starts_with("localhost:") {
        // If it does, try to parse as a socket address
        match parse_socket_address(value) {
            Ok(socket_addr) => {
                // If socket address parsing succeeds, return a URL with the "http" scheme and the
                // socket address as the host
                let url_str = format!("http://{}", socket_addr);
                return Url::parse(&url_str).map_err(|_| URLParsingError::Parse(url_str));
            }
            Err(_) => return Err(URLParsingError::Parse(value.to_string())),
        }
    }

    match Url::parse(value) {
        Ok(url) => {
            // Check if the scheme is http or https
            if url.scheme() != "https" && url.scheme() != "http" {
                return Err(URLParsingError::InvalidScheme(url.scheme().to_string()));
            }
            Ok(url)
        }
        Err(_) => {
            // If URL parsing fails, try to parse as a socket address
            match parse_socket_address(value) {
                Ok(socket_addr) => {
                    // If socket address parsing succeeds, return a URL with the "http" scheme and
                    // the socket address as the host
                    let url_str = format!("http://{}", socket_addr);
                    Url::parse(&url_str).map_err(|_| URLParsingError::Parse(url_str))
                }
                Err(_) => Err(URLParsingError::Parse(value.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_socket_address_empty() {
        let result = parse_socket_address("");
        assert!(matches!(result, Err(SocketAddressParsingError::Empty)));
    }

    #[test]
    fn test_parse_socket_address_port_only() {
        let result = parse_socket_address(":8080").unwrap();
        assert_eq!(result, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080));
    }

    #[test]
    fn test_parse_socket_address_localhost_port() {
        let result = parse_socket_address("localhost:8080").unwrap();
        assert_eq!(result, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080));
    }

    #[test]
    fn test_parse_socket_address_port_as_value() {
        let result = parse_socket_address("8080").unwrap();
        assert_eq!(result, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080));
    }

    #[test]
    fn test_parse_url_empty() {
        let result = parse_url("");
        assert!(matches!(result, Err(URLParsingError::Empty)));
    }

    #[test]
    fn test_parse_url_valid() {
        let result = parse_url("http://localhost:8080").unwrap();
        assert_eq!(result, Url::parse("http://localhost:8080").unwrap());
    }

    #[test]
    fn test_parse_https_url_valid() {
        let result = parse_url("https://localhost:8080").unwrap();
        assert_eq!(result, Url::parse("https://localhost:8080").unwrap());
    }

    #[test]
    fn test_parse_url_invalid() {
        let result = parse_url("invalid_url");
        assert!(matches!(result, Err(URLParsingError::Parse(_))));
    }

    #[test]
    fn test_parse_url_unsupported_scheme() {
        let result = parse_url("ftp://localhost:8080");
        assert!(matches!(result, Err(URLParsingError::InvalidScheme(_))));
    }

    #[test]
    fn test_parse_url_socket_address_port_only() {
        let result = parse_url(":8080").unwrap();
        assert_eq!(result, Url::parse("http://127.0.0.1:8080").unwrap());
    }

    #[test]
    fn test_parse_url_socket_address_localhost_port() {
        let result = parse_url("localhost:8080").unwrap();
        assert_eq!(result, Url::parse("http://127.0.0.1:8080").unwrap());
    }

    #[test]
    fn test_parse_url_socket_address_ip_port_as_value() {
        let result = parse_url("127.0.0.1:8080").unwrap();
        assert_eq!(result, Url::parse("http://127.0.0.1:8080").unwrap());
    }

    #[test]
    fn test_parse_url_socket_address_port_as_value() {
        let result = parse_url("8080").unwrap();
        assert_eq!(result, Url::parse("http://127.0.0.1:8080").unwrap());
    }
}
