//! CLI progress reporting.
//! It adapts usecase progress events to JSON lines and terminal progress bars.

use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;

use crate::base::progress_status::ProgressStatus;
use crate::userinterfaces::common::output::{emit_json_line, format_eta};

pub(crate) struct ProgressReporter {
    json_mode: bool,
    quiet: bool,
    bar: Option<ProgressBar>,
}

impl ProgressReporter {
    pub(crate) fn new(json_mode: bool, quiet: bool) -> Self {
        let bar = if quiet {
            None
        } else {
            let pb = ProgressBar::new(1);
            pb.set_style(
                ProgressStyle::with_template(
                    "{msg:20} [{bar:30.cyan/blue}] {percent:>3}% ETA {eta_precise}",
                )
                .unwrap_or_else(|_| ProgressStyle::default_bar()),
            );
            Some(pb)
        };

        Self {
            json_mode,
            quiet,
            bar,
        }
    }

    pub(crate) fn callback(&self) -> impl FnMut(ProgressStatus) + '_ {
        let json_mode = self.json_mode;
        let quiet = self.quiet;
        let bar = self.bar.clone();

        move |event: ProgressStatus| {
            if json_mode {
                emit_json_line(json!({
                    "type": "progress",
                    "phase": event.phase,
                    "current": event.current,
                    "total": event.total,
                    "pct": event.pct,
                    "etaSeconds": event.eta_seconds,
                    "ratePerSecond": event.rate_per_second,
                }));
            }

            if quiet {
                return;
            }

            if let Some(pb) = &bar {
                let total = event.total.max(1);
                pb.set_length(total);
                pb.set_position(event.current.min(total));
                let eta_text = event
                    .eta_seconds
                    .map(format_eta)
                    .unwrap_or_else(|| "~?".to_string());
                pb.set_message(format!("{} {}", event.phase, eta_text));
            }
        }
    }

    pub(crate) fn finish(&self) {
        if let Some(pb) = &self.bar {
            pb.finish_and_clear();
        }
    }
}
