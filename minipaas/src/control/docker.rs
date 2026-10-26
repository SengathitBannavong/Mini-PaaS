use bollard::Docker;
use bollard::models::{ContainerCreateBody, ContainerInspectResponse};
use bollard::plugin::ContainerSummary;
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptionsBuilder, InspectContainerOptions,
    ListContainersOptionsBuilder, RemoveContainerOptionsBuilder, StartContainerOptions,
};
use futures_util::TryStreamExt;
use std::collections::HashMap;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// One started Replica
pub struct Replica {
    pub id: String,
    pub url: String,
}

/// What a deploy asks for, after defaults are filled in
pub struct Spec {
    pub digest: String,
    pub port: u16,
}

/// Pull `image` again and read its digest and port (`port` given, or the single `EXPOSE`)
pub async fn pull(docker: &Docker, image: &str, port: Option<u16>) -> Result<Spec, Error> {
    let opts = CreateImageOptionsBuilder::default()
        .from_image(image)
        .build();
    docker
        .create_image(Some(opts), None, None)
        .try_collect::<Vec<_>>()
        .await?;

    let img = docker.inspect_image(image).await?;
    let digests = img.repo_digests.unwrap_or_default();
    let prefix = format!("{}@", repo_of(image));
    let digest = digests
        .iter()
        .find(|d| d.starts_with(&prefix))
        .or(digests.first())
        .cloned()
        .ok_or("image has no digest (built locally? push it to a registry)")?;
    let port = match port {
        Some(p) => p,
        None => {
            let exposed = img.config.and_then(|c| c.exposed_ports).unwrap_or_default();
            match exposed.as_slice() {
                // "8080/tcp" -> 8080
                [one] => one.split('/').next().unwrap_or("").parse()?,
                [] => return Err("no port given and image has no EXPOSE".into()),
                _ => {
                    return Err(format!("image exposes several ports {exposed:?}, give one").into());
                }
            }
        }
    };
    Ok(Spec { digest, port })
}

/// "localhost:5000/app:v1" -> "localhost:5000/app" (keeps a registry port, drops tag or digest)
fn repo_of(image: &str) -> &str {
    let image = image.split('@').next().unwrap_or(image);
    match image.rsplit_once(':') {
        Some((repo, tag)) if !tag.contains('/') => repo,
        _ => image,
    }
}

/// Start one container of `spec` for `host` on the default bridge network
pub async fn start_replica(
    docker: &Docker,
    host: &str,
    health_path: &str,
    spec: &Spec,
) -> Result<Replica, Error> {
    let body = ContainerCreateBody {
        image: Some(spec.digest.clone()),
        // labels are the source of truth for running Replicas (ADR 0003)
        labels: Some(HashMap::from([
            ("minipaas.host".to_string(), host.to_string()),
            ("minipaas.port".to_string(), spec.port.to_string()),
            ("minipaas.health_path".to_string(), health_path.to_string()),
        ])),
        ..Default::default()
    };
    let id = docker
        .create_container(None::<CreateContainerOptions>, body)
        .await?
        .id;
    docker
        .start_container(&id, None::<StartContainerOptions>)
        .await?;

    let info = docker
        .inspect_container(&id, None::<InspectContainerOptions>)
        .await?;
    match replica_url(&info, spec.port) {
        Some(url) => Ok(Replica { id, url }),
        None => {
            remove(docker, &id).await.ok();
            Err("container has no IP on bridge".into())
        }
    }
}

/// Where the router sends traffic for this container (ADR 0002).
// Linux + bridge only. Docker Desktop (Mac/Win) or remote host -> publish port and return host:port here
fn replica_url(info: &ContainerInspectResponse, port: u16) -> Option<String> {
    let ip = info
        .network_settings
        .as_ref()?
        .networks
        .as_ref()?
        .get("bridge")?
        .ip_address
        .as_ref()?;
    (!ip.is_empty()).then(|| format!("http://{ip}:{port}"))
}

/// Stop and delete a container
pub async fn remove(docker: &Docker, id: &str) -> Result<(), Error> {
    let opts = RemoveContainerOptionsBuilder::default().force(true).build();
    Ok(docker.remove_container(id, Some(opts)).await?)
}

/// Containers we created (found by label); `host` = only that App's. Includes stopped ones.
pub async fn list(docker: &Docker, host: Option<&str>) -> Result<Vec<ContainerSummary>, Error> {
    let label = match host {
        Some(h) => format!("minipaas.host={h}"),
        None => "minipaas.host".to_string(),
    };
    let filters = HashMap::from([("label".to_string(), vec![label])]);
    let opts = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    Ok(docker.list_containers(Some(opts)).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs Docker and network: cargo test -- --ignored
    #[tokio::test]
    #[ignore = "test try pull image -> start replica -> check health -> remove contrainer"]
    async fn pulls_starts_and_reaches_container_by_ip() {
        let docker = Docker::connect_with_local_defaults().unwrap();
        let spec = pull(&docker, "traefik/whoami:latest", None).await.unwrap();
        assert!(spec.digest.contains("@sha256:"));
        assert_eq!(spec.port, 80); // from EXPOSE

        let r = start_replica(&docker, "t.localhost", "/", &spec)
            .await
            .unwrap();
        let b = crate::router::Backend::new(&r.url);
        let mut ready = false;
        for _ in 0..50 {
            if crate::health::check_backend(&b.host, b.port, "/").await {
                ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        remove(&docker, &r.id).await.unwrap();
        assert!(ready, "{} never answered", r.url);
    }

    #[test]
    fn repo_of_drops_tag_and_digest_but_keeps_registry_port() {
        assert_eq!(
            repo_of("localhost:5000/testapp:v1"),
            "localhost:5000/testapp"
        );
        assert_eq!(repo_of("localhost:5000/testapp"), "localhost:5000/testapp");
        assert_eq!(repo_of("traefik/whoami@sha256:abc"), "traefik/whoami");
        assert_eq!(repo_of("nginx"), "nginx");
    }
}
