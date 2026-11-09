pub mod health;
pub mod middleware;
pub mod proxy;

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;

use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone)]
pub struct Backend {
    pub host: String,
    pub port: u16,
    pub url: String,
    pub healthy: bool,
    /// In-flight requests on this Replica. Clones share it
    pub in_flight: Arc<AtomicUsize>,
}

impl Backend {
    pub fn new(url: &str) -> Self {
        let trimmed = url.trim_start_matches("http://");
        let parts: Vec<&str> = trimmed.split(':').collect();
        let host = parts[0].to_string();
        let port: u16 = parts.get(1).unwrap_or(&"80").parse().unwrap_or(80);
        Backend {
            host,
            port,
            url: url.to_string(),
            healthy: true,
            in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug)]
pub struct Route {
    pub host: String,
    pub health_path: String, // for control plane
    backends: RwLock<Vec<Backend>>,
    current: AtomicUsize,
}

impl Route {
    pub fn new(host: &str, healthy_path: &str, backend_url: &Vec<&str>) -> Self {
        let backends: Vec<Backend> = backend_url.iter().map(|url| Backend::new(url)).collect();
        Route {
            host: host.to_string(),
            health_path: healthy_path.to_string(),
            backends: RwLock::new(backends),
            current: AtomicUsize::new(0),
        }
    }

    /// Get the next healthy backend using round-robin
    pub fn next_backend(&self) -> Option<Backend> {
        let backends = self.backends.read().unwrap();
        let healthy: Vec<&Backend> = backends.iter().filter(|b| b.healthy).collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = self.current.fetch_add(1, Ordering::Relaxed) % healthy.len();
        Some(healthy[idx].clone())
    }

    /// In-flight requests over all Replicas of this Route
    pub fn in_flight(&self) -> usize {
        let backends = self.backends.read().unwrap();
        backends
            .iter()
            .map(|b| b.in_flight.load(Ordering::SeqCst))
            .sum()
    }

    /// Get all backends (for health checking)
    pub fn get_backends(&self) -> Vec<Backend> {
        self.backends.read().unwrap().clone()
    }

    /// Update the health status of a backend
    pub fn set_backend_health(&self, url: &str, healthy: bool) {
        let mut backends = self.backends.write().unwrap();
        if let Some(backend) = backends.iter_mut().find(|b| b.url == url) {
            if backend.healthy != healthy {
                let status = if healthy { "healthy" } else { "unhealthy" };
                println!("  [health] {} is now {}", url, status);
            }
            backend.healthy = healthy;
        }
    }
}

pub struct Router {
    routes: RwLock<HashMap<String, Arc<Route>>>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            routes: RwLock::new(HashMap::new()),
        }
    }

    /// Add or replace the route for `host`.
    /// A request sees either the old route or the new one, never a mix
    pub fn set_route(&self, host: &str, health_path: &str, backend_urls: Vec<&str>) {
        let route = Arc::new(Route::new(host, health_path, &backend_urls));
        println!(" Route set: {} -> {}", host, backend_urls.join(", "));
        self.routes
            .write()
            .unwrap()
            .insert(host.to_lowercase(), route);
    }

    pub fn find_route(&self, host: &str) -> Option<Arc<Route>> {
        // "Users.Localhost:3000" -> "users.localhost"
        let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
        self.routes.read().unwrap().get(&host).cloned()
    }

    pub fn all_routes(&self) -> Vec<Arc<Route>> {
        self.routes.read().unwrap().values().cloned().collect()
    }
}

/// Counts on In-flight request on a Replica until dropped
pub struct InFlight(Arc<AtomicUsize>);

impl InFlight {
    pub fn new(backend: &Backend) -> Self {
        backend.in_flight.fetch_add(1, Ordering::SeqCst);
        InFlight(backend.in_flight.clone())
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_counts_untils_guard_dropped_and_survives_route_swap() {
        let r = Router::new();
        r.set_route("a.localhost", "/", vec!["http://127.0.0.1:8081"]);
        let blue = r.find_route("a.localhost").unwrap();
        let g1 = InFlight::new(&blue.next_backend().unwrap());
        let g2 = InFlight::new(&blue.next_backend().unwrap());
        assert_eq!(blue.in_flight(), 2);

        r.set_route("a.localhost", "/", vec!["http://127.0.0.1:8082"]); // swap to green
        assert_eq!(r.find_route("a.localhost").unwrap().in_flight(), 0);
        assert_eq!(blue.in_flight(), 2);

        drop(g1);
        drop(g2);
        assert_eq!(blue.in_flight(), 0);
    }

    #[test]
    fn routes_by_host_ignoring_port_and_case() {
        let r = Router::new();
        r.set_route("users.localhost", "/", vec!["http://127.0.0.1:8081"]);
        assert!(r.find_route("Users.Localhost:3000").is_some());
        assert!(r.find_route("orders.localhost").is_none());
        r.set_route(
            "users.localhost",
            "/",
            vec!["http://127.0.0.1:8082", "http://127.0.0.1:8083"],
        );
        let routes: Vec<String> = r
            .find_route("Users.Localhost:3000")
            .unwrap()
            .get_backends()
            .into_iter()
            .map(|b| b.url)
            .collect();
        assert_eq!(routes, ["http://127.0.0.1:8082", "http://127.0.0.1:8083"]);
    }
}
