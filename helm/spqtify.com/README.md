# spqtify.com Helm chart

This chart deploys both services in this repository:

- `api`
- `embed-image-service`

## Quick start

1. Create the API secret:

```bash
kubectl -n spqtify create secret generic spqtify-api-secrets \
  --from-literal=B2_BUCKET_ID=your-bucket-id \
  --from-literal=B2_APPLICATION_KEY_ID=your-key-id \
  --from-literal=B2_APPLICATION_KEY=your-key
```

2. Find the latest image tags in the GHCR UI:

- Open `https://github.com/users/iAverages/packages/container/package/spqtify.com`.
- Open the `spqtify.com/api` package and copy the newest immutable tag from the package versions list.
- Open the `spqtify.com/embed-image-service` package and copy the newest immutable tag from the package versions list.

3. Create `values.yaml` with this minimal example and set image tags/host:

```yaml
api:
  image:
    tag: production-<git-sha>
  existingSecret: spqtify-api-secrets

embed-image-service:
  image:
    tag: production-<git-sha>

ingress:
  enabled: true
  host: spqtify.example.com
```

4. Install from OCI:

```bash
helm upgrade --install spqtify.com oci://ghcr.io/iaverages/charts/spqtify.com \
  --version 1.0.0 \
  --namespace spqtify \
  --create-namespace \
  -f values.yaml
```
