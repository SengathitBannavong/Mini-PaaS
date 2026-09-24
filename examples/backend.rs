use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn handle(
	req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {

	let path = req.uri().path();
	let body = format! (
		"Hello from the backend!\nYou requested: {} {}\n",
		req.method(),
		path
	);
	Ok(Response::new(Full::new(Bytes::from(body))))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = SocketAddr::from(([127,0,0,1],8080));
	let listener = TcpListener::bind(addr).await?;
	println!("Backend server listening on http://{}", addr);
	
	loop {
		let (stream, _) = listener.accept().await?;
		let io = TokioIo::new(stream);
		tokio::task::spawn(async move {
			if let Err(err) = http1::Builder::new()
				.serve_connection(io, service_fn(handle))
				.await
			{
				eprintln!("Error: {:?}", err);
			}
		});
	}
}
