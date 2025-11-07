FROM ghcr.io/foundry-rs/foundry:latest

# Switch to root to install packages
USER root

# Install jq for parsing JSON in deployment script
# Foundry image is Debian-based
RUN apt-get update && apt-get install -y jq && rm -rf /var/lib/apt/lists/*

# Switch back to foundry user
USER foundry

WORKDIR /workspace
