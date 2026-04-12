//! CLI rendering for command results.
//! This module owns human-readable wording and JSON result shapes.

use serde_json::json;

use crate::domains::timelock::CURRENT_MACHINE_PROFILE_ID;
use crate::usecases::{calibrate, inspect, lock, unlock, verify};
use crate::userinterfaces::common::output::{emit_json_line, format_binary_size, format_eta};

pub(crate) fn render_lock_result(
    response: &lock::LockResponse,
    json_mode: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    if json_mode {
        emit_json_line(json!({
            "type": "result",
            "command": "lock",
            "output": response.output_path,
            "iterations": response.iterations,
            "hardwareProfile": response.hardware_profile,
            "payloadBytes": response.payload_bytes
        }));
    }

    if !quiet {
        println!("Timelocked file created");
        println!("Output: {}", response.output_path.display());
        println!("Iterations: {}", response.iterations);
        println!("Hardware profile: {}", response.hardware_profile);
    }

    Ok(())
}

pub(crate) fn render_unlock_result(
    response: &unlock::UnlockResponse,
    json_mode: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    if json_mode {
        match &response.recovered_payload {
            unlock::RecoveredPayload::File { path } => {
                emit_json_line(json!({
                    "type": "result",
                    "command": "unlock",
                    "output": path,
                    "recoveredBytes": response.recovered_bytes
                }));
            }
            unlock::RecoveredPayload::Text { text } => {
                emit_json_line(json!({
                    "type": "result",
                    "command": "unlock",
                    "message": text,
                    "recoveredBytes": response.recovered_bytes
                }));
            }
        }
    }

    if !quiet {
        println!("Unlock complete");
        println!("Payload authentication and recovery: OK");
        match &response.recovered_payload {
            unlock::RecoveredPayload::File { path } => {
                println!("Output file: {}", path.display());
            }
            unlock::RecoveredPayload::Text { text } => {
                println!("Recovered message:");
                println!("{text}");
            }
        }
        println!(
            "Recovered size: {}",
            format_binary_size(response.recovered_bytes)
        );
    }

    Ok(())
}

pub(crate) fn render_inspect_result(
    response: &inspect::InspectResponse,
    json_mode: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    if json_mode {
        emit_json_line(json!({
            "type": "result",
            "command": "inspect",
            "path": response.path,
            "formatVersion": response.format_version,
            "payloadBytes": response.payload_len,
            "header": response.header,
            "estimatedDurationOnProfileSeconds": response.estimated_duration_on_profile_seconds
        }));
    }

    if !quiet {
        println!("File: {}", response.path.display());
        println!("Format: timelocked/v{}", response.format_version);
        println!("Created: {}", response.header.created_at);
        println!("Payload: {}", format_binary_size(response.payload_len));
        println!(
            "Hardware profile: {}",
            response.header.timelock_params.hardware_profile
        );
        println!("Delay params:");
        println!(
            "  iterations: {}",
            response.header.timelock_params.iterations
        );
        if let Some(secs) = response.estimated_duration_on_profile_seconds {
            println!("  estimatedDurationOnProfile: {}", format_eta(secs));
        }
    }

    Ok(())
}

pub(crate) fn render_calibrate_result(
    response: &calibrate::CalibrateResponse,
    json_mode: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    if json_mode {
        emit_json_line(json!({
            "type": "result",
            "command": "calibrate",
            "hardwareProfile": CURRENT_MACHINE_PROFILE_ID,
            "iterationsPerSecond": response.iterations_per_second
        }));
    }

    if !quiet {
        println!("Current machine calibration complete");
        println!("Iterations/second: {}", response.iterations_per_second);
    }

    Ok(())
}

pub(crate) fn render_verify_result(
    response: &verify::VerifyResponse,
    json_mode: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    if json_mode {
        emit_json_line(json!({
            "type": "result",
            "command": "verify",
            "path": response.path,
            "chunkCount": response.chunk_count,
            "payloadPlaintextBytes": response.payload_plaintext_bytes,
            "status": "ok"
        }));
    }

    if !quiet {
        println!("Structural verification OK");
        println!("Validated file structure and recoverability metadata.");
        println!("Use unlock for full payload authentication and recovery.");
        println!("File: {}", response.path.display());
        println!("Chunks: {}", response.chunk_count);
        println!(
            "Payload plaintext size: {}",
            format_binary_size(response.payload_plaintext_bytes)
        );
    }

    Ok(())
}
