# Releasing the spqtify.com Helm chart

## Release flow

2. Update `helm/spqtify.com/Chart.yaml` `version` if you are shipping a new chart version.
3. Commit and push the chart changes.
4. Create and push a chart release tag matching `spqtify.com-chart-v*` (for example `spqtify.com-chart-v1.2.3`).
5. Wait for `.github/workflows/publish-helm-chart.yaml` to complete.
6. Verify the chart was published to `oci://ghcr.io/iaverages/charts`.

