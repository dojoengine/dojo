//! Middleware that proxies requests at a specified URI to internal
//! RPC method calls.
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::header::{ACCEPT, CONTENT_TYPE};
use hyper::http::HeaderValue;
use hyper::{Body, Method, Request, Response, Uri};
use jsonrpsee_core::error::Error as RpcError;
use jsonrpsee_core::JsonRawValue;
use jsonrpsee_types::{Id, RequestSer};
use tower::{Layer, Service};
use url::form_urlencoded;

use crate::transport::http;

/// Layer that applies [`DevnetProxy`] which proxies the `GET /path` requests to
/// specific RPC method calls and that strips the response.
///
/// See [`DevnetProxy`] for more details.
#[derive(Debug, Clone)]
pub struct DevnetProxyLayer {}

impl DevnetProxyLayer {
    /// Creates a new [`DevnetProxyLayer`].
    ///
    /// See [`DevnetProxy`] for more details.
    pub fn new() -> Result<Self, RpcError> {
        Ok(Self {})
    }
}
impl<S> Layer<S> for DevnetProxyLayer {
    type Service = DevnetProxy<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DevnetProxy::new(inner).expect("Path already validated in DevnetProxyLayer; qed")
    }
}

/// Proxy `GET /path` requests to the specified RPC method calls.
///
/// # Request
///
/// The `GET /path` requests are modified into valid `POST` requests for
/// calling the RPC method. This middleware adds appropriate headers to the
/// request, and completely modifies the request `BODY`.
///
/// # Response
///
/// The response of the RPC method is stripped down to contain only the method's
/// response, removing any RPC 2.0 spec logic regarding the response' body.
#[derive(Debug, Clone)]
pub struct DevnetProxy<S> {
    inner: S,
}

impl<S> DevnetProxy<S> {
    /// Creates a new [`DevnetProxy`].
    ///
    /// The request `GET /path` is redirected to the provided method.
    /// Fails if the path does not start with `/`.
    pub fn new(inner: S) -> Result<Self, RpcError> {
        Ok(Self { inner })
    }
}

impl<S> Service<Request<Body>> for DevnetProxy<S>
where
    S: Service<Request<Body>, Response = Response<Body>>,
    S::Response: 'static,
    S::Error: Into<Box<dyn Error + Send + Sync>> + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = Box<dyn Error + Send + Sync + 'static>;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let query = req.uri().query();
        let path = req.uri().path();

        let (params, method) = match path {
            "/account_balance" => get_account_balance(query),
            _ => (JsonRawValue::from_string("{}".to_string()).unwrap(), "".to_string()),
        };

        if !method.is_empty() {
            // RPC methods are accessed with `POST`.
            *req.method_mut() = Method::POST;
            // Precautionary remove the URI.
            *req.uri_mut() = Uri::from_static("/");

            // Requests must have the following headers:
            req.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            req.headers_mut().insert(ACCEPT, HeaderValue::from_static("application/json"));

            // Adjust the body to reflect the method call.
            let body = Body::from(
                serde_json::to_string(&RequestSer::borrowed(
                    &Id::Number(0),
                    &method,
                    Some(&params),
                ))
                .expect("Valid request; qed"),
            );
            req = req.map(|_| body);
        }

        // Call the inner service and get a future that resolves to the response.
        let fut = self.inner.call(req);

        // Adjust the response if needed.
        let res_fut = async move {
            let res = fut.await.map_err(|err| err.into())?;

            if method.is_empty() {
                return Ok(res);
            }

            let body = res.into_body();
            let bytes = hyper::body::to_bytes(body).await?;

            #[derive(serde::Deserialize, Debug)]
            struct RpcPayload<'a> {
                #[serde(borrow)]
                result: &'a serde_json::value::RawValue,
            }

            let response = if let Ok(payload) = serde_json::from_slice::<RpcPayload<'_>>(&bytes) {
                http::response::ok_response(payload.result.to_string())
            } else {
                http::response::internal_error()
            };

            Ok(response)
        };

        Box::pin(res_fut)
    }
}

fn get_account_balance(query: Option<&str>) -> (Box<JsonRawValue>, std::string::String) {
    let default = String::new();

    let query = query.unwrap_or(&default);
    let params: HashMap<_, _> = form_urlencoded::parse(query.as_bytes()).into_owned().collect();

    let address = params.get("contract_address").unwrap_or(&default);
    let unit = params.get("unit").unwrap_or(&default);

    let json_string = format!(r#"{{"address":"{}", "unit":"{}"}}"#, address, unit);
    (JsonRawValue::from_string(json_string).unwrap(), "dev_accountBalance".to_string())
}
