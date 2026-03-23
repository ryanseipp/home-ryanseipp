#!/usr/bin/env bash
set -euo pipefail

# Register workload entries for local development.
# Run after the SPIRE agent is healthy.
#
# Application services run as the current user (unix:uid selector).
# Infrastructure services run as root inside containers (unix:uid:0).
#
# To regenerate the bootstrap cert (if expired or lost):
#   openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 \
#     -keyout local/spire/agent-bootstrap.key \
#     -out local/spire/agent-bootstrap.crt \
#     -days 3650 -nodes -subj "/CN=spire-agent-bootstrap" \
#     -addext "keyUsage=digitalSignature"

TRUST_DOMAIN="home.ryanseipp.com"
DEV_UID="$(id -u)"

# Get the agent's actual SPIFFE ID (x509pop generates a hash-based ID)
AGENT_ID=$(docker compose exec spire-server \
    /opt/spire/bin/spire-server agent list -output json \
    | jq -r '.agents[0].id.trust_domain + .agents[0].id.path')

echo "Agent SPIFFE ID: spiffe://${AGENT_ID}"
echo "Registering workload entries for UID ${DEV_UID}..."

# --- Application service entries (host processes, current UID) ---

docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/gateway" \
    -parentID "spiffe://${AGENT_ID}" \
    -selector "unix:uid:${DEV_UID}" \
    -dns gateway \
    || true

docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/identity" \
    -parentID "spiffe://${AGENT_ID}" \
    -selector "unix:uid:${DEV_UID}" \
    -dns identity \
    || true

# --- Infrastructure service entries (container processes, UID 0) ---

docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/postgres" \
    -parentID "spiffe://${AGENT_ID}" \
    -selector "unix:uid:0" \
    -dns postgres \
    -dns localhost \
    || true

docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/scylla" \
    -parentID "spiffe://${AGENT_ID}" \
    -selector "unix:uid:0" \
    -dns scylla \
    -dns localhost \
    || true

docker compose exec spire-server \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "spiffe://${TRUST_DOMAIN}/kafka" \
    -parentID "spiffe://${AGENT_ID}" \
    -selector "unix:uid:0" \
    -dns kafka \
    -dns localhost \
    || true

echo "Done. Workload entries registered."
echo ""
echo "Set the socket path before running services:"
echo "  export SPIFFE_ENDPOINT_SOCKET=tcp://localhost:8443"
