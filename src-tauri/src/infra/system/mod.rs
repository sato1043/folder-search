use std::process::Command;

use crate::domain::system::{GpuInfo, SystemInfo};

/// システム情報を検出する
pub fn detect_system_info() -> SystemInfo {
    let total_ram_mb = detect_ram_mb();
    let gpus = detect_gpus();

    // macOS は Metal が自動有効、cuda フィーチャー有効時は CUDA が利用可能
    let gpu_inference_available = cfg!(target_os = "macos") || cfg!(feature = "cuda");

    SystemInfo {
        total_ram_mb,
        gpus,
        gpu_inference_available,
    }
}

/// システムRAMをMB単位で取得する
fn detect_ram_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / (1024 * 1024)
}

/// GPU情報を検出する（プラットフォーム別）
/// 検出に失敗した場合は空のVecを返す
fn detect_gpus() -> Vec<GpuInfo> {
    let result = detect_gpus_platform();
    result.unwrap_or_default()
}

/// Windows: wmic でGPU情報を取得する
#[cfg(target_os = "windows")]
fn detect_gpus_platform() -> Result<Vec<GpuInfo>, String> {
    let output = Command::new("wmic")
        .args([
            "path",
            "win32_VideoController",
            "get",
            "Name,AdapterRAM",
            "/format:csv",
        ])
        .output()
        .map_err(|e| format!("wmic実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("wmic異常終了".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();

    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // CSV形式: Node,AdapterRAM,Name
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            let adapter_ram_str = parts[1].trim();
            let name = parts[2].trim().to_string();

            if name.is_empty() {
                continue;
            }

            let vram_bytes: u64 = adapter_ram_str.parse().unwrap_or(0);
            let vram_mb = vram_bytes / (1024 * 1024);

            gpus.push(GpuInfo { name, vram_mb });
        }
    }

    Ok(gpus)
}

/// macOS: system_profiler でGPU情報を取得する
#[cfg(target_os = "macos")]
fn detect_gpus_platform() -> Result<Vec<GpuInfo>, String> {
    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .map_err(|e| format!("system_profiler実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("system_profiler異常終了".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSONパース失敗: {}", e))?;

    let mut gpus = Vec::new();

    if let Some(displays) = json.get("SPDisplaysDataType").and_then(|v| v.as_array()) {
        for display in displays {
            let name = display
                .get("sppci_model")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown GPU")
                .to_string();

            // VRAMの取得を試みる
            // Apple Siliconは "spdisplays_gmem" が存在しない場合がある
            let vram_mb = display
                .get("spdisplays_vram")
                .or_else(|| display.get("spdisplays_gmem"))
                .and_then(|v| v.as_str())
                .and_then(|s| parse_macos_vram(s))
                .unwrap_or(0);

            gpus.push(GpuInfo { name, vram_mb });
        }
    }

    Ok(gpus)
}

/// macOSのVRAM文字列をMB単位にパースする（例: "8 GB", "1536 MB"）
#[cfg(target_os = "macos")]
fn parse_macos_vram(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(gb_str) = s.strip_suffix("GB").or_else(|| s.strip_suffix("gb")) {
        gb_str.trim().parse::<u64>().ok().map(|v| v * 1024)
    } else if let Some(mb_str) = s.strip_suffix("MB").or_else(|| s.strip_suffix("mb")) {
        mb_str.trim().parse::<u64>().ok()
    } else {
        s.parse::<u64>().ok()
    }
}

/// Linux: lspci + nvidia-smi でGPU情報を取得する
#[cfg(target_os = "linux")]
fn detect_gpus_platform() -> Result<Vec<GpuInfo>, String> {
    // まず nvidia-smi を試みる（NVIDIA GPU）
    if let Ok(gpus) = detect_gpus_nvidia_smi() {
        if !gpus.is_empty() {
            return Ok(gpus);
        }
    }

    // nvidia-smiが使えない場合は lspci でGPU名のみ取得する
    detect_gpus_lspci()
}

/// nvidia-smi でNVIDIA GPU情報を取得する
#[cfg(target_os = "linux")]
fn detect_gpus_nvidia_smi() -> Result<Vec<GpuInfo>, String> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .map_err(|e| format!("nvidia-smi実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("nvidia-smi異常終了".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, ',').collect();
        if parts.len() == 2 {
            let name = parts[0].trim().to_string();
            let vram_mb: u64 = parts[1].trim().parse().unwrap_or(0);
            gpus.push(GpuInfo { name, vram_mb });
        }
    }

    Ok(gpus)
}

/// lspci でGPU名を取得する（VRAMは取得できない）
#[cfg(target_os = "linux")]
fn detect_gpus_lspci() -> Result<Vec<GpuInfo>, String> {
    let output = Command::new("lspci")
        .output()
        .map_err(|e| format!("lspci実行失敗: {}", e))?;

    if !output.status.success() {
        return Err("lspci異常終了".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();

    for line in stdout.lines() {
        // VGA compatible controller または 3D controller の行を探す
        if line.contains("VGA compatible controller")
            || line.contains("3D controller")
            || line.contains("Display controller")
        {
            // "XX:XX.X VGA compatible controller: GPU Name" の形式
            if let Some(pos) = line.find(": ") {
                let name = line[pos + 2..].trim().to_string();
                gpus.push(GpuInfo { name, vram_mb: 0 });
            }
        }
    }

    Ok(gpus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_ram_returns_positive() {
        let ram = detect_ram_mb();
        assert!(ram > 0, "システムRAMは正の値を返すべき");
    }

    #[test]
    fn test_detect_system_info_structure() {
        let info = detect_system_info();
        assert!(info.total_ram_mb > 0);
        // gpu_inference_available はプラットフォームとフィーチャーに依存する
        // macOS または cuda フィーチャー有効時は true
        let expected = cfg!(target_os = "macos") || cfg!(feature = "cuda");
        assert_eq!(info.gpu_inference_available, expected);
    }

    #[test]
    fn test_detect_gpus_does_not_panic() {
        // GPU検出はエラーでも空Vecを返しパニックしない
        let gpus = detect_gpus();
        // 結果の型が正しいことだけ確認
        let _: Vec<GpuInfo> = gpus;
    }
}
