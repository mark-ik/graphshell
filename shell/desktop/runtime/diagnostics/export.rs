use super::*;

impl DiagnosticsState {
    pub(crate) fn export_snapshot_json(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!(
            "diagnostics-{}.json",
            Self::export_timestamp_secs()
        ));
        let payload = serde_json::to_string_pretty(&self.snapshot_json_value())
            .map_err(|e| format!("failed to serialize diagnostics JSON: {e}"))?;
        fs::write(&path, payload)
            .map_err(|e| format!("failed to write diagnostics JSON {}: {e}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn export_snapshot_svg(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!("diagnostics-{}.svg", Self::export_timestamp_secs()));
        fs::write(&path, self.engine_svg())
            .map_err(|e| format!("failed to write diagnostics SVG {}: {e}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn export_bridge_spike_json(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!(
            "bridge-spike-{}.json",
            Self::export_timestamp_secs()
        ));
        let payload = serde_json::to_string_pretty(&self.bridge_spike_measurement_value())
            .map_err(|e| format!("failed to serialize bridge spike JSON: {e}"))?;
        fs::write(&path, payload)
            .map_err(|e| format!("failed to write bridge spike JSON {}: {e}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn export_backend_telemetry_report_json(&self) -> Result<PathBuf, String> {
        let dir = Self::export_dir()?;
        let path = dir.join(format!(
            "backend-telemetry-{}.json",
            Self::export_timestamp_secs()
        ));
        let payload = serde_json::to_string_pretty(&self.backend_telemetry_report_value())
            .map_err(|e| format!("failed to serialize backend telemetry JSON: {e}"))?;
        fs::write(&path, payload).map_err(|e| {
            format!(
                "failed to write backend telemetry JSON {}: {e}",
                path.display()
            )
        })?;
        Ok(path)
    }
}

