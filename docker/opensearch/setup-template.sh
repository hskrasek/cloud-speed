#!/bin/bash
set -e

OPENSEARCH_URL="${OPENSEARCH_URL:-http://localhost:9200}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Waiting for OpenSearch to be ready..."
until curl -s "${OPENSEARCH_URL}/_cluster/health" | grep -q '"status":"green"\|"status":"yellow"'; do
    echo "  OpenSearch not ready, waiting..."
    sleep 2
done

echo "OpenSearch is ready!"
echo "Creating index template..."

curl -X PUT "${OPENSEARCH_URL}/_index_template/cloud-speed-template" \
    -H "Content-Type: application/json" \
    -d @"${SCRIPT_DIR}/index-template.json"

echo ""
echo "Index template created successfully!"
echo ""
echo "Verify with: curl ${OPENSEARCH_URL}/_index_template/cloud-speed-template"
