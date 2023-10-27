use {
    bollard::{image::CreateImageOptions, Docker},
    futures::StreamExt,
    std::collections::HashSet,
    tokio::sync::Mutex,
};

/// Component that starts and manages docker containers. Mainly used to ensure
/// that all containers spawned for a test will also be terminated.
pub struct ContainerRegistry {
    docker: Docker,
    containers: Mutex<HashSet<String>>,
}

impl Default for ContainerRegistry {
    fn default() -> Self {
        Self {
            docker: Docker::connect_with_socket_defaults().unwrap(),
            containers: Default::default(),
        }
    }
}

impl ContainerRegistry {
    /// Spawns the given container and remembers it for later termination.
    pub async fn start(&self, container_id: String) {
        self.containers.lock().await.insert(container_id.clone());
        self.docker
            .start_container::<&str>(&container_id, None)
            .await
            .unwrap();
    }

    /// Kills all containers that are still managed by the registry and
    /// "forgets" them afterwards.
    pub async fn kill_all(&self) {
        let mut containers = self.containers.lock().await;
        futures::future::join_all(containers.iter().map(|container_id| async {
            Self::terminate_container(&self.docker, container_id).await;
        }))
        .await;
        containers.clear();
    }

    /// Terminates the given container without updating the registry.
    async fn terminate_container(docker: &Docker, container_id: &String) {
        if let Err(err) = docker.kill_container::<&str>(container_id, None).await {
            tracing::error!(?err, ?container_id, "could not kill container");
        }
    }

    /// Pulls the given image.
    pub async fn pull_image(&self, image: &str) {
        tracing::info!(image, "pulling docker image");
        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );
        // consume stream until we are done
        while stream.next().await.is_some() {}
    }
}
