---
name: check-deployment
description: "Check if a RIG preview deployment is actually running and healthy. Verifies CI triggered, images exist, pods started, and services respond. Use when a PR deployment seems broken or after pushing code."
user_invocable: true
---

# Check Deployment

Verify that a RIG preview deployment is actually running and healthy. Takes a PR number or deployment name as argument.

## Context

Deployments happen automatically via CI/CD. This skill is for **checking** if something went wrong, not for creating deployments. Common failure modes:
- CI/Deploy workflow didn't trigger after a push
- Image doesn't exist or uses wrong tag
- Pods didn't start (quota exceeded, image pull errors)
- RIG API reported success but pods are actually failing

## Instructions

### 1. Parse argument

Accept a PR number (e.g., `204`) or deployment name (e.g., `pr204`, `pr204b`). If just a number, the deployment name is `pr{N}`.

### 2. Load RIG API key

```bash
eval "$(grep RIG_API_KEY /workspace/.env)"
```

### 3. Check CI/Deploy status

If a PR number is given, check if the deploy workflow ran for the latest commit:

```bash
# Get latest commit on PR
gh pr view {N} --repo MinBZK/regelrecht-mvp --json headRefOid,headRefName -q '{sha: .headRefOid, branch: .headRefName}'

# Check if deploy workflow triggered for that commit
gh run list --repo MinBZK/regelrecht-mvp --branch {branch} --workflow deploy.yml --json headSha,status,conclusion --limit 3
```

Report:
- Whether the latest commit has a deploy workflow run
- Whether it succeeded or is still running
- If no run exists: **the deploy was never triggered** — suggest re-pushing or closing/reopening the PR

### 4. Check pod logs

Verify pods are actually running by checking for recent log output:

```bash
curl -s -H "X-API-Key: $RIG_API_KEY" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/logs/regel-k4c?deployment={name}&lines=5"
```

For each component (editor, harvester-admin, harvester-worker):
- **Has recent logs**: pod is running
- **Empty logs (0 lines)**: pod is NOT running — likely image pull error or quota issue

### 5. Check image availability

If pods aren't running, verify the images exist:

```bash
# Check what tags exist for this PR
for pkg in regelrecht-mvp regelrecht-admin regelrecht-harvester-worker; do
  gh api "/orgs/MinBZK/packages/container/${pkg}/versions" \
    --jq ".[] | select(.metadata.container.tags | any(test(\"pr-{N}\"))) | .metadata.container.tags"
done
```

Important: The cluster pulls via a Harbor mirror. `ghcr.io` is blocked directly. Only `pr-{N}` tags are reliably available through Harbor. `sha-{hash}` tags may NOT work.

### 6. Report summary

Present a clear status table:

| Check | Status |
|-------|--------|
| CI/Deploy triggered | yes/no |
| Build succeeded | yes/no |
| Images exist (pr-{N} tag) | yes/no |
| editor pod running | yes/no |
| harvester-admin pod running | yes/no |
| harvester-worker pod running | yes/no |

If any check fails, explain:
- What went wrong
- Why it likely happened
- What to do about it (re-push, wait for CI, check quota, etc.)

### 7. If asked to fix

If the user asks to fix a broken deployment:
- **CI didn't trigger**: Suggest re-pushing (`git commit --allow-empty -m "ci: retrigger" && git push`)
- **Images missing**: Wait for CI to complete, then check again
- **Pods not starting**: Check quota by listing all active deployments, suggest cleaning up stale ones
- **Only as last resort**: Create a manual deployment — but **ALWAYS ask the user for confirmation first**. Explain what you're about to do and why, and wait for approval before making any RIG API calls. Manual deploys are not the normal workflow; the CI/CD pipeline should handle this automatically.

When creating a manual deployment (after user approval), use `pr-{N}` tags (NEVER `sha-` tags):
```bash
curl -s -X POST -H "X-API-Key: $RIG_API_KEY" -H "Content-Type: application/json" \
  "https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl/api/projects/regel-k4c/:upsert-deployment" \
  -d '{"deploymentName": "{name}", "cloneFrom": "regelrecht", "components": [
    {"reference": "editor", "image": "ghcr.io/minbzk/regelrecht-mvp:pr-{N}"},
    {"reference": "harvester-admin", "image": "ghcr.io/minbzk/regelrecht-admin:pr-{N}"},
    {"reference": "harvester-worker", "image": "ghcr.io/minbzk/regelrecht-harvester-worker:pr-{N}"}
  ]}'
```

**Then wait 60-90 seconds and verify pods started via logs. Do NOT report success until logs confirm pods are running.**
