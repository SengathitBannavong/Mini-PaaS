use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response};

pub mod cors;
pub mod headers;
pub mod logging;
pub mod ratelimit;

/// The context that flows through the middleware pipeline
pub struct RequestContext {
    pub client_ip: String,
    pub start_time: std::time::Instant,
    pub origin: Option<String>, // from req.headers().get("Origin") in main.rs
}

/// The middleware trait - implement this to create new middleware
pub trait Middleware: Send + Sync {
    /// Called before the request is forwarded to the backend
    /// Return the (possibly modified) request, or return a response
    /// to short-circuit the pipeline and skip the backend entirely
    fn on_request(
        &self,
        req: Request<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Result<Request<BoxBody<Bytes, hyper::Error>>, Response<BoxBody<Bytes, hyper::Error>>>;

    /// Called after the response comes back from the backend.
    /// Return the (possibly modified) response.
    fn on_response(
        &self,
        resp: Response<BoxBody<Bytes, hyper::Error>>,
        _ctx: &RequestContext,
    ) -> Response<BoxBody<Bytes, hyper::Error>> {
        // Default: pass through unchanged
        resp
    }

    /// A human-readable name for this middleware (used in logging)
    fn name(&self) -> &str;
}

/// a pipeline of middleware that process requests in order
pub struct MiddlewarePipeline {
    middlewares: Vec<Box<dyn Middleware>>,
}

impl MiddlewarePipeline {
    pub fn new() -> Self {
        MiddlewarePipeline {
            middlewares: Vec::new(),
        }
    }

    pub fn add(&mut self, middleware: Box<dyn Middleware>) {
        println!("  Middleware added: {}", middleware.name());
        self.middlewares.push(middleware);
    }

    /// Process a request through all middleware before forwarding
    pub fn process_request(
        &self,
        mut req: Request<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Result<Request<BoxBody<Bytes, hyper::Error>>, Response<BoxBody<Bytes, hyper::Error>>> {
        for mw in &self.middlewares {
            match mw.on_request(req, ctx) {
                Ok(modified_req) => req = modified_req,
                Err(response) => return Err(response),
            }
        }
        Ok(req)
    }

    /// Process a response through all middleware in reverse order
    pub fn process_response(
        &self,
        mut resp: Response<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Response<BoxBody<Bytes, hyper::Error>> {
        for mw in self.middlewares.iter().rev() {
            resp = mw.on_response(resp, ctx);
        }
        resp
    }
}
