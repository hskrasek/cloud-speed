#!/bin/bash
set -e

# Configuration (can be overridden by environment variables)
OPENSEARCH_URL="${OPENSEARCH_URL:-http://opensearch:9200}"
OPENSEARCH_INDEX="${OPENSEARCH_INDEX:-cloud-speed}"

# Generate index name with date suffix for time-based indices
INDEX_DATE=$(date +%Y.%m)
FULL_INDEX="${OPENSEARCH_INDEX}-${INDEX_DATE}"

# Run the speed test and capture JSON output
echo "[$(date -Iseconds)] Starting speed test..."

if RESULT=$(cloud-speed --json 2>/dev/null); then
    echo "[$(date -Iseconds)] Speed test completed successfully"

    # Send to OpenSearch
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        -X POST "${OPENSEARCH_URL}/${FULL_INDEX}/_doc" \
        -H "Content-Type: application/json" \
        -d "${RESULT}")

    if [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
        echo "[$(date -Iseconds)] Successfully sent to OpenSearch (HTTP ${HTTP_CODE})"
    else
        echo "[$(date -Iseconds)] ERROR: Failed to send to OpenSearch (HTTP ${HTTP_CODE})"
        # Save failed result for potential retry
        echo "${RESULT}" >> /var/log/cloud-speed/failed_results.jsonl
    fi
else
    EXIT_CODE=$?
    echo "[$(date -Iseconds)] ERROR: Speed test failed with exit code ${EXIT_CODE}"
fi
