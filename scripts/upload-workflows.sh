#!/bin/bash
# Upload workflow files to GitHub via the REST API
# This works with PATs that have 'repo' scope (no 'workflow' scope needed for API)

set -e
REPO="monch1962/webread"
BRANCH="master"

upload_workflow() {
    local file="$1"
    local path=".github/workflows/$file"
    local content=$(cat "$path")
    local encoded=$(echo -n "$content" | base64 -w0)
    local msg="ci: add $file"

    echo "Creating $path ..."
    gh api "repos/$REPO/contents/$path" \
        -X PUT \
        -f message="$msg" \
        -f content="$encoded" \
        -f branch="$BRANCH"
    echo "  done"
}

upload_workflow "ci.yml"
upload_workflow "release.yml"

echo ""
echo "✅ Workflows uploaded."
echo "   View: https://github.com/$REPO/actions"
echo "   Trigger release: git tag v0.1.0 && git push origin v0.1.0"
