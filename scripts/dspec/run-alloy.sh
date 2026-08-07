#!/bin/sh

set -eu

ALLOY_VERSION="6.2.0"
ALLOY_SHA256="6b8c1cb5bc93bedfc7c61435c4e1ab6e688a242dc702a394628d9a9801edb78d"
ALLOY_ASSET="org.alloytools.alloy.dist.jar"
ALLOY_URL="https://github.com/AlloyTools/org.alloytools.alloy/releases/download/v${ALLOY_VERSION}/${ALLOY_ASSET}"

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
MODEL_PATH=${1:-"${SCRIPT_DIR}/ime-yen-input.als"}
CACHE_BASE=${ALLOY_CACHE_ROOT:-"${HOME}/Library/Caches/karukan/alloy"}
JAR_PATH=${ALLOY_JAR:-"${CACHE_BASE}/${ALLOY_VERSION}/${ALLOY_ASSET}"}

if [ ! -f "$JAR_PATH" ]; then
  mkdir -p "$(dirname -- "$JAR_PATH")"
  DOWNLOAD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/karukan-alloy-download.XXXXXX")
  trap 'rm -rf "$DOWNLOAD_DIR"' EXIT INT TERM
  curl --fail --location --output "${DOWNLOAD_DIR}/${ALLOY_ASSET}" "$ALLOY_URL"
  mv "${DOWNLOAD_DIR}/${ALLOY_ASSET}" "$JAR_PATH"
fi

ACTUAL_SHA256=$(shasum -a 256 "$JAR_PATH" | awk '{print $1}')
if [ "$ACTUAL_SHA256" != "$ALLOY_SHA256" ]; then
  echo "Alloy jar SHA-256 mismatch: expected ${ALLOY_SHA256}, got ${ACTUAL_SHA256}" >&2
  exit 1
fi

OUTPUT_DIR=$(mktemp -d "${TMPDIR:-/tmp}/karukan-alloy-ime-yen-input.XXXXXX")
java --enable-native-access=ALL-UNNAMED -jar "$JAR_PATH" \
  exec -c '*' -t json -o "$OUTPUT_DIR" "$MODEL_PATH"
node "${SCRIPT_DIR}/check-alloy-result.mjs" "${OUTPUT_DIR}/receipt.json"
echo "Alloy evidence: ${OUTPUT_DIR}"
