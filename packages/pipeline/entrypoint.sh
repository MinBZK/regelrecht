#!/bin/sh
set -e

REPO_PATH="${REGULATION_REPO_PATH:-/data/regulation-repo}"

if [ ! -d "$REPO_PATH/.git" ]; then
    echo "Initializing local git repo at $REPO_PATH"
    git init "$REPO_PATH"
    git -C "$REPO_PATH" config user.email "harvester@regelrecht.local"
    git -C "$REPO_PATH" config user.name "Harvest Worker"
fi

exec regelrecht-harvest-worker
