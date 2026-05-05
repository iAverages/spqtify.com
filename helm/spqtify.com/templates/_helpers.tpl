{{- define "spqtify.com.name" -}}
{{- default .Chart.Name .Values.nameOverride | replace "." "-" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "spqtify.com.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | replace "." "-" | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := include "spqtify.com.name" . -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | replace "." "-" | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | replace "." "-" | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "spqtify.com.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" -}}
{{- end -}}

{{- define "spqtify.com.labels" -}}
helm.sh/chart: {{ include "spqtify.com.chart" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "spqtify.com.selectorLabels" -}}
app.kubernetes.io/name: {{ include "spqtify.com.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "spqtify.com.api.fullname" -}}
{{- printf "%s-api" (include "spqtify.com.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "spqtify.com.embedImageService.fullname" -}}
{{- printf "%s-embed-image-service" (include "spqtify.com.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
