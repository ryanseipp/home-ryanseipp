#!/usr/bin/env bash
set -euo pipefail

# Register workload entries for local development.
# Run this after `docker compose up -d` and the SPIRE agent is healthy.
#
# Both services run as the current user, so we use the unix:uid selector
# to match processes by UID.

TRUST_DOMAIN="home.ryanseipp.com"
DEV_UID="$(id -u)"

echo "Registering workload entries for UID ${DEV_UID}..."

# Generate a join token and attach the agent
TOKEN=$(docker compose exec spire-server \
    /opt/spire/bin/spire-server token generate \
    -spiffeID "spiffe://${TRUST_DOMAIN}/agent" \
    -output json | jq -r '.value')

echo "Join token generated. Attaching agent..."
docker compose exec spire-agent \
    /opt/spire/bin/spire-agent run -joinToken "${TOKEN}" &>/dev/null &

# Wait for agent to be healthy
echo "Waiting for agent..."
for i in $(seq 1 30); do
    if docker compose exec spire-agent \
        /opt/spire/bin/spire-agent healthcheck &>/dev/null; then
        break
    fi
    sleep 1
done

# Gateway workload entry
docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/gateway" \
    -parentID "spiffe://${TRUST_DOMAIN}/agent" \
    -selector "unix:uid:${DEV_UID}" \
    || true

# Identity workload entry
docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/identity" \
    -parentID "spiffe://${TRUST_DOMAIN}/agent" \
    -selector "unix:uid:${DEV_UID}" \
    || true

echo "Done. Workload entries registered."
echo ""
echo "Set the socket path before running services:"
echo "  export SPIFFE_ENDPOINT_SOCKET=unix:///tmp/spire-agent/public/api.sock"
