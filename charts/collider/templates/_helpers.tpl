{{/*
Expand the name of the chart.
*/}}
{{- define "collider.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "collider.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "collider.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "collider.labels" -}}
helm.sh/chart: {{ include "collider.chart" . }}
{{ include "collider.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
environment: {{ .Values.global.environment }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "collider.selectorLabels" -}}
app.kubernetes.io/name: {{ include "collider.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Common labels for postgres
*/}}
{{- define "collider.postgres.labels" -}}
{{ include "collider.labels" . }}
app.kubernetes.io/component: postgres
{{- end }}

{{/*
Selector labels for postgres
*/}}
{{- define "collider.postgres.selectorLabels" -}}
{{ include "collider.selectorLabels" . }}
app.kubernetes.io/component: postgres
{{- end }}

{{/*
Common labels for dragonfly
*/}}
{{- define "collider.dragonfly.labels" -}}
{{ include "collider.labels" . }}
app.kubernetes.io/component: dragonfly
{{- end }}

{{/*
Selector labels for dragonfly
*/}}
{{- define "collider.dragonfly.selectorLabels" -}}
{{ include "collider.selectorLabels" . }}
app.kubernetes.io/component: dragonfly
{{- end }}

{{/*
Common labels for app
*/}}
{{- define "collider.app.labels" -}}
{{ include "collider.labels" . }}
app.kubernetes.io/component: app
{{- end }}

{{/*
Selector labels for app
*/}}
{{- define "collider.app.selectorLabels" -}}
{{ include "collider.selectorLabels" . }}
app.kubernetes.io/component: app
{{- end }}

{{/*
Common labels for monitoring components
*/}}
{{- define "collider.monitoring.labels" -}}
{{ include "collider.labels" . }}
app.kubernetes.io/component: monitoring
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "collider.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "collider.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Database URL
*/}}
{{- define "collider.databaseUrl" -}}
{{- if .Values.global.environment | eq "local" }}
postgres://{{ .Values.postgres.auth.username }}:{{ .Values.postgres.auth.password }}@{{ include "collider.fullname" . }}-postgres:5432/{{ .Values.postgres.auth.database }}
{{- else }}
postgres://{{ .Values.postgres.auth.username }}:{{ .Values.postgres.auth.password }}@{{ include "collider.fullname" . }}-postgres.{{ .Release.Namespace }}.svc.cluster.local:5432/{{ .Values.postgres.auth.database }}
{{- end }}
{{- end }}

{{/*
Redis URL
*/}}
{{- define "collider.redisUrl" -}}
{{- if .Values.global.environment | eq "local" }}
redis://:{{ .Values.dragonfly.auth.password }}@{{ include "collider.fullname" . }}-dragonfly:6379
{{- else }}
redis://:{{ .Values.dragonfly.auth.password }}@{{ include "collider.fullname" . }}-dragonfly.{{ .Release.Namespace }}.svc.cluster.local:6379
{{- end }}
{{- end }}

{{/*
Jaeger endpoint
*/}}
{{- define "collider.jaegerEndpoint" -}}
{{- if .Values.global.environment | eq "local" }}
http://{{ include "collider.fullname" . }}-jaeger:14268/api/traces
{{- else }}
http://{{ include "collider.fullname" . }}-jaeger.{{ .Release.Namespace }}.svc.cluster.local:14268/api/traces
{{- end }}
{{- end }}

{{/*
Resource limits and requests
*/}}
{{- define "collider.resources" -}}
{{- if .resources }}
resources:
  {{- if .resources.limits }}
  limits:
    {{- if .resources.limits.cpu }}
    cpu: {{ .resources.limits.cpu }}
    {{- end }}
    {{- if .resources.limits.memory }}
    memory: {{ .resources.limits.memory }}
    {{- end }}
  {{- end }}
  {{- if .resources.requests }}
  requests:
    {{- if .resources.requests.cpu }}
    cpu: {{ .resources.requests.cpu }}
    {{- end }}
    {{- if .resources.requests.memory }}
    memory: {{ .resources.requests.memory }}
    {{- end }}
  {{- end }}
{{- end }}
{{- end }}