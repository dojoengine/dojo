pub(crate) mod response {
    use jsonrpsee_types::error::{ErrorCode, ErrorResponse};
    use jsonrpsee_types::Id;

    const JSON: &str = "application/json; charset=utf-8";

    /// Create a response for json internal error.
    pub(crate) fn internal_error() -> hyper::Response<hyper::Body> {
        let error = serde_json::to_string(&ErrorResponse::borrowed(
            ErrorCode::InternalError.into(),
            Id::Null,
        ))
        .expect("built from known-good data; qed");

        from_template(hyper::StatusCode::INTERNAL_SERVER_ERROR, error, JSON)
    }

    /// Create a response body.
    fn from_template<S: Into<hyper::Body>>(
        status: hyper::StatusCode,
        body: S,
        content_type: &'static str,
    ) -> hyper::Response<hyper::Body> {
        hyper::Response::builder()
			.status(status)
			.header("content-type", hyper::header::HeaderValue::from_static(content_type))
			.body(body.into())
			// Parsing `StatusCode` and `HeaderValue` is infalliable but
			// parsing body content is not.
			.expect("Unable to parse response body for type conversion")
    }

    /// Create a valid JSON response.
    pub(crate) fn ok_response(body: String) -> hyper::Response<hyper::Body> {
        from_template(hyper::StatusCode::OK, body, JSON)
    }
}
