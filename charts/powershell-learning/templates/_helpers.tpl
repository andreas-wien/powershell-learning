{{/*
Expand the name of the chart.
*/}}
{{- define "powershell-learning.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "powershell-learning.fullname" -}}
{{- if and .Values .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default (and .Chart .Chart.Name | default "powershell-learning") (and .Values .Values.nameOverride) }}
{{- if and .Release .Release.Name (contains $name .Release.Name) }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" (and .Release .Release.Name | default "release") $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "powershell-learning.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "powershell-learning.labels" -}}
helm.sh/chart: {{ include "powershell-learning.chart" . }}
{{ include "powershell-learning.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "powershell-learning.selectorLabels" -}}
app.kubernetes.io/name: {{ include "powershell-learning.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "powershell-learning.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "powershell-learning.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}