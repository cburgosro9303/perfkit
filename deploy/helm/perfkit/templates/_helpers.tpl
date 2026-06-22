{{/*
Helpers de nombres y etiquetas del chart perfkit.
*/}}

{{- define "perfkit.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
Nombre completo (release + chart), recortado a 63 chars (límite de DNS de K8s).
*/}}
{{- define "perfkit.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "perfkit.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
Etiquetas comunes a todos los recursos.
*/}}
{{- define "perfkit.labels" -}}
helm.sh/chart: {{ include "perfkit.chart" . }}
app.kubernetes.io/name: {{ include "perfkit.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/part-of: perfkit
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end -}}

{{/*
Etiquetas de selección por componente. Uso: include "perfkit.selectorLabels" (dict "ctx" . "component" "coordinator")
*/}}
{{- define "perfkit.selectorLabels" -}}
app.kubernetes.io/name: {{ include "perfkit.name" .ctx }}
app.kubernetes.io/instance: {{ .ctx.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end -}}

{{/*
Nombres por componente.
*/}}
{{- define "perfkit.coordinator.fullname" -}}
{{- printf "%s-coordinator" (include "perfkit.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "perfkit.worker.fullname" -}}
{{- printf "%s-worker" (include "perfkit.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "perfkit.target.fullname" -}}
{{- printf "%s-target" (include "perfkit.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "perfkit.scenario.configMapName" -}}
{{- printf "%s-scenario" (include "perfkit.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
URL interna del coordinator (FQDN del Service en el namespace del release).
*/}}
{{- define "perfkit.coordinator.url" -}}
{{- printf "http://%s.%s.svc.cluster.local:%d" (include "perfkit.coordinator.fullname" .) .Release.Namespace (int .Values.coordinator.service.port) -}}
{{- end -}}

{{/*
FQDN del Service headless de workers (para el descubrimiento DNS del coordinator).
*/}}
{{- define "perfkit.worker.serviceFqdn" -}}
{{- printf "%s.%s.svc.cluster.local" (include "perfkit.worker.fullname" .) .Release.Namespace -}}
{{- end -}}
