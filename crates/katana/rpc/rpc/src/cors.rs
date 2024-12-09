use http::HeaderValue;
use tower::Layer;
use tower_http::cors::{self, Any};
pub use tower_http::cors::{AllowHeaders, AllowMethods};

/// Layer that applies the [`Cors`] middleware which adds headers for [CORS][mdn].
///
/// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS
/// [`Cors`]: cors::Cors
#[derive(Debug, Clone, Default)]
pub struct Cors(cors::CorsLayer);

impl Cors {
    pub fn new() -> Self {
        Self::default()
    }

    /// A permissive configuration:
    ///
    /// - All request headers allowed.
    /// - All methods allowed.
    /// - All origins allowed.
    /// - All headers exposed.
    pub fn permissive() -> Self {
        Self(cors::CorsLayer::permissive())
    }

    /// A very permissive configuration:
    ///
    /// - **Credentials allowed.**
    /// - The method received in `Access-Control-Request-Method` is sent back as an allowed method.
    /// - The origin of the preflight request is sent back as an allowed origin.
    /// - The header names received in `Access-Control-Request-Headers` are sent back as allowed
    ///   headers.
    /// - No headers are currently exposed, but this may change in the future.
    pub fn very_permissive() -> Self {
        Self(cors::CorsLayer::very_permissive())
    }

    pub fn allow_origins(self, origins: impl Into<AllowOrigins>) -> Self {
        Self(self.0.allow_origin(origins.into()))
    }

    pub fn allow_methods(self, methods: impl Into<AllowMethods>) -> Self {
        Self(self.0.allow_methods(methods))
    }

    pub fn allow_headers(self, headers: impl Into<AllowHeaders>) -> Self {
        Self(self.0.allow_headers(headers))
    }
}

impl<S> Layer<S> for Cors {
    type Service = cors::Cors<S>;

    fn layer(&self, inner: S) -> Self::Service {
        self.0.layer(inner)
    }
}

const WILDCARD: HeaderValue = HeaderValue::from_static("*");

/// Holds configuration for how to set the [`Access-Control-Allow-Origin`][mdn] header.
///
/// This is just a lightweight wrapper of [`cors::AllowOrigin`] that doesn't fail when a wildcard,
/// `*`, is passed to [`cors::AllowOrigin::list`]. See [`cors::AllowOrigin`] for more details.
///
/// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin
#[derive(Debug, Clone, Default)]
pub struct AllowOrigins(cors::AllowOrigin);

impl AllowOrigins {
    /// Allow any origin by sending a wildcard (`*`).
    pub fn any() -> Self {
        Self(cors::AllowOrigin::any())
    }

    /// Set a single allowed origin.
    pub fn exact(origin: HeaderValue) -> Self {
        Self(cors::AllowOrigin::exact(origin))
    }

    /// Allow any origin, by mirroring the request origin.
    pub fn mirror_request() -> Self {
        Self(cors::AllowOrigin::mirror_request())
    }

    /// Set multiple allowed origins.
    ///
    /// This will not return an error if a wildcard, `*`, is in the list.
    pub fn list<I>(origins: I) -> Self
    where
        I: IntoIterator<Item = HeaderValue>,
    {
        let origins = origins.into_iter().collect::<Vec<_>>();
        if origins.iter().any(|o| o == WILDCARD) {
            Self(cors::AllowOrigin::any())
        } else {
            Self(cors::AllowOrigin::list(origins))
        }
    }
}

impl From<cors::AllowOrigin> for AllowOrigins {
    fn from(value: cors::AllowOrigin) -> Self {
        Self(value)
    }
}

impl From<AllowOrigins> for cors::AllowOrigin {
    fn from(value: AllowOrigins) -> Self {
        value.0
    }
}

impl From<Any> for AllowOrigins {
    fn from(_: Any) -> Self {
        Self(cors::AllowOrigin::any())
    }
}

impl From<HeaderValue> for AllowOrigins {
    fn from(val: HeaderValue) -> Self {
        Self(cors::AllowOrigin::exact(val))
    }
}

impl<const N: usize> From<[HeaderValue; N]> for AllowOrigins {
    fn from(arr: [HeaderValue; N]) -> Self {
        Self::list(arr)
    }
}

impl From<Vec<HeaderValue>> for AllowOrigins {
    fn from(vec: Vec<HeaderValue>) -> Self {
        Self::list(vec)
    }
}
