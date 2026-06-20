use crate::device_runtime::{DeviceCycleResult, DisplayAction, DisplayDecision, DisplayTarget};
use crate::diagnostics::{
    append_event_to_file, daily_log_path, remove_logs_older_than, DiagnosticEvent, DiagnosticLevel,
    DiagnosticLogWrite, LOGS_DIR,
};
use crate::model::LocalDate;
use crate::power::NextRunPlan;

const DIAGNOSTIC_LOG_KEEP_DAYS: u8 = 14;

pub struct MountedDiagnosticLog {
    path: std::path::PathBuf,
    run_epoch_seconds: u64,
}

impl MountedDiagnosticLog {
    pub fn new(date: LocalDate, run_epoch_seconds: u64) -> Self {
        let _ = remove_logs_older_than(LOGS_DIR, date, DIAGNOSTIC_LOG_KEEP_DAYS);
        Self {
            path: daily_log_path(date),
            run_epoch_seconds,
        }
    }

    pub fn info(
        &mut self,
        time: u64,
        event: &str,
        message: &str,
        data: impl FnOnce(DiagnosticEvent) -> DiagnosticEvent,
    ) {
        self.write(data(DiagnosticEvent::new(
            time,
            self.run_epoch_seconds,
            DiagnosticLevel::Info,
            event,
            message,
        )));
    }

    pub fn warn(
        &mut self,
        time: u64,
        event: &str,
        message: &str,
        data: impl FnOnce(DiagnosticEvent) -> DiagnosticEvent,
    ) {
        self.write(data(DiagnosticEvent::new(
            time,
            self.run_epoch_seconds,
            DiagnosticLevel::Warn,
            event,
            message,
        )));
    }

    pub fn error(
        &mut self,
        time: u64,
        event: &str,
        message: &str,
        data: impl FnOnce(DiagnosticEvent) -> DiagnosticEvent,
    ) {
        self.write(data(DiagnosticEvent::new(
            time,
            self.run_epoch_seconds,
            DiagnosticLevel::Error,
            event,
            message,
        )));
    }

    pub fn record_cycle(
        &mut self,
        time: u64,
        cycle: &DeviceCycleResult,
        next_run_plan: &NextRunPlan,
    ) {
        self.info(time, "cycle", "device cycle completed", |event| {
            event.with_data("outcome", format!("{:?}", cycle.outcome))
        });
        self.info(time, "sync", "sync decision resolved", |event| {
            let event = event
                .with_data("action", format!("{:?}", cycle.sync_decision.action))
                .with_data("cause", format!("{:?}", cycle.sync_decision.cause))
                .with_data("attempted", cycle.sync_attempted)
                .with_data("succeeded", cycle.sync_succeeded);
            append_sync_error_data(event, cycle)
        });
        if cycle.sync_error.is_some() {
            self.warn(time, "sync", "sync failed", |event| {
                append_sync_error_data(event, cycle)
            });
        }
        self.info(time, "display", "display decision resolved", |event| {
            append_display_decision_data(event, &cycle.display_decision)
                .with_data("refresh_attempted", cycle.refresh_attempted)
                .with_data("refresh_succeeded", cycle.refresh_succeeded)
        });
        self.info(time, "next", "next run scheduled", |event| {
            event
                .with_data("at", next_run_plan.next_run_epoch_seconds)
                .with_data("wait", next_run_plan.wait_seconds)
                .with_data(
                    "mode",
                    if cycle.battery.externally_powered() {
                        "restart"
                    } else {
                        "deep-sleep"
                    },
                )
        });
    }

    fn write(&self, event: DiagnosticEvent) {
        match append_event_to_file(&self.path, &event) {
            DiagnosticLogWrite::Written => {}
            error => {
                log::warn!(target: "epaper_album", "diagnostic log write failed: {error:?}");
            }
        }
    }
}

fn append_sync_error_data(event: DiagnosticEvent, cycle: &DeviceCycleResult) -> DiagnosticEvent {
    let Some(error) = cycle.sync_error.as_ref() else {
        return event;
    };
    let event = event.with_data("error", error.to_string());
    let Some(report) = cycle.sync_error_report.as_ref() else {
        return event;
    };
    let event = event
        .with_data("code", report.code.clone())
        .with_data("category", report.category.clone())
        .with_data("message", report.message.clone())
        .with_data("detail", report.detail.clone());
    match &report.stage {
        Some(stage) => event.with_data("stage", stage.clone()),
        None => event,
    }
}

fn append_display_decision_data(
    event: DiagnosticEvent,
    decision: &DisplayDecision,
) -> DiagnosticEvent {
    match &decision.action {
        DisplayAction::Keep => event
            .with_data("action", "Keep")
            .with_data("cause", format!("{:?}", decision.cause)),
        DisplayAction::Refresh(DisplayTarget::Photo {
            date,
            image,
            caption,
        }) => event
            .with_data("action", "RefreshPhoto")
            .with_data("cause", format!("{:?}", decision.cause))
            .with_data("date", date.to_string())
            .with_data("image", image.clone())
            .with_data("caption", caption.clone()),
        DisplayAction::Refresh(DisplayTarget::Page {
            date,
            title,
            message,
            hint,
            detail,
        }) => event
            .with_data("action", "RefreshPage")
            .with_data("cause", format!("{:?}", decision.cause))
            .with_data("date", date.to_string())
            .with_data("title", title.clone())
            .with_data("message", message.clone())
            .with_data("hint", hint.clone())
            .with_data("detail", detail.clone()),
    }
}
