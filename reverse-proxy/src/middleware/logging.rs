use crate::middleware::Middleware;
use crate::middleware::RequestContext;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response};

pub struct LoggingMiddleware;

impl Middleware for LoggingMiddleware {
    fn on_request(
        &self,
        req: Request<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Result<Request<BoxBody<Bytes, hyper::Error>>, Response<BoxBody<Bytes, hyper::Error>>> {
        println!(
            "[{}] --> {} {} (from {})",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            req.method(),
            req.uri().path(),
            ctx.client_ip
        );
        Ok(req)
    }

    fn on_response(
        &self,
        resp: Response<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Response<BoxBody<Bytes, hyper::Error>> {
        let elapsed = ctx.start_time.elapsed();
        println!(
            "[{}] <-- {} ({:.2}ms)",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            resp.status(),
            elapsed.as_secs_f64() * 1000.0,
        );
        resp
    }

    fn name(&self) -> &str {
        "logging"
    }
}
