#!/bin/sh
set -eu

api="http://server.in.svart.io/gitea/api/v1/repos/${GITHUB_REPOSITORY}"
tag="${GITHUB_REF_NAME}"

release_json="$(curl -fsS \
  -H "Authorization: token ${GITEA_TOKEN}" \
  "${api}/releases/tags/${tag}" || true)"

release_id="$(printf '%s' "$release_json" | sed -n 's/.*"id":[[:space:]]*\([0-9][0-9]*\).*/\1/p')"

if [ -z "$release_id" ]; then
  release_json="$(curl -fsS -X POST \
    -H "Authorization: token ${GITEA_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"tag_name\":\"${tag}\",\"name\":\"${tag}\",\"draft\":false,\"prerelease\":false}" \
    "${api}/releases")"
  release_id="$(printf '%s' "$release_json" | sed -n 's/.*"id":[[:space:]]*\([0-9][0-9]*\).*/\1/p')"
fi

if [ -z "$release_id" ]; then
  echo "Could not determine release id for ${tag}" >&2
  exit 1
fi

for asset in "$@"; do
  asset_name="$(basename "$asset")"
  curl -fsS -X POST \
    -H "Authorization: token ${GITEA_TOKEN}" \
    -F "attachment=@${asset}" \
    "${api}/releases/${release_id}/assets?name=${asset_name}"
done
