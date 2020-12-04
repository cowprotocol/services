#!/bin/bash

set -euo pipefail

if [ "$TRAVIS_PULL_REQUEST" = "false" ]; then
    echo "$DOCKER_PASSWORD" | docker login -u "$DOCKER_NAME" --password-stdin
    # Build and tag image
    docker build -t $DOCKERHUB_PROJECT -f docker/Dockerfile .
    docker tag $DOCKERHUB_PROJECT gnosispm/$DOCKERHUB_PROJECT:$1
    docker push gnosispm/$DOCKERHUB_PROJECT:$1

    if [ "$1" == "main" ] && [ -n "$AUTODEPLOY_URL" ]; then
      # Notifying webhook
      curl -s  \
      --output /dev/null \
      --write-out "%{http_code}" \
      -H "Content-Type: application/json" \
      -d '{"push_data": {"tag": "'$AUTODEPLOY_TAG'" }}' \
      -X POST \
      $AUTODEPLOY_URL
    fi
fi
