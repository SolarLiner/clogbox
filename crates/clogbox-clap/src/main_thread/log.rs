#![cfg(feature = "log")]
use std::ffi::CString;
use std::sync::mpsc;

use clack_extensions::log as log_ext;
use clack_extensions::timer as timer_ext;
use clack_plugin::host::{HostMainThreadHandle, HostSharedHandle};
use log::LevelFilter;

struct LogMessage {
    level: log_ext::LogSeverity,
    message: String,
}

pub(super) struct ClapLogger {
    tx: mpsc::Sender<LogMessage>,
}

fn log_level_to_clap(level: log::Level) -> clack_extensions::log::LogSeverity {
    use log_ext::LogSeverity;
    match level {
        log::Level::Error => LogSeverity::Error,
        log::Level::Warn => LogSeverity::Warning,
        log::Level::Info => LogSeverity::Info,
        _ => LogSeverity::Debug,
    }
}

impl log::Log for ClapLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        let data = format!(
            "[{}:{}] {}",
            record.module_path().unwrap_or("<unknown>"),
            record.line().unwrap_or(0),
            record.args()
        );
        let level = log_level_to_clap(record.level());
        eprintln!("[log] sending {level:?} message to host: {data}");
        let _ = self.tx.send(LogMessage { level, message: data });
    }

    fn flush(&self) {}
}

pub(super) struct LogExtension {
    timer_id: timer_ext::TimerId,
    rx: mpsc::Receiver<LogMessage>,
}

impl LogExtension {
    pub(super) fn on_timer(&mut self, host: HostSharedHandle, timer_id: clack_extensions::timer::TimerId) {
        eprintln!("log::on_timer: timer_id={}", timer_id);
        if self.timer_id != timer_id {
            return;
        }
        let Some(log) = host.get_extension::<log_ext::HostLog>() else {
            return;
        };
        for LogMessage { level, message } in self.rx.try_iter() {
            let msg = match CString::new(message.into_bytes()) {
                Ok(string) => string,
                Err(err) => {
                    log::debug!("logging: {err}");
                    continue;
                }
            };
            log.log(&host, level, &msg);
        }
    }
}

fn register_logging_timer(handle: &mut HostMainThreadHandle) -> Option<clack_extensions::timer::TimerId> {
    let Some(host_timer) = handle.get_extension::<clack_extensions::timer::HostTimer>() else {
        log::debug!("Host does not support timers");
        return None;
    };
    let timer_id = match host_timer.register_timer(handle, 100) {
        Ok(timer_id) => timer_id,
        Err(err) => {
            log::debug!("Host did not register timer: {err}");
            return None;
        }
    };
    log::debug!("log::register_logging_timer: timer_id={}", timer_id);
    Some(timer_id)
}

pub(super) fn init(host: &mut HostMainThreadHandle) -> Option<LogExtension> {
    let (tx, rx) = mpsc::channel();
    let Some(timer_id) = register_logging_timer(host) else {
        init_env_logger();
        log::error!("CLAP timer not registered, setting up default logger instead");
        return None;
    };
    if let Err(err) = log::set_boxed_logger(Box::new(ClapLogger { tx })) {
        init_env_logger();
        log::error!("logging setup error: {}", err);
        return None;
    }
    log::debug!("log::init: successful");
    Some(LogExtension { timer_id, rx })
}

#[track_caller]
fn init_env_logger() {
    let caller = core::panic::Location::caller();
    eprintln!(
        "{}:{}: Init default logger (using env_logger)",
        caller.file(),
        caller.line()
    );
    env_logger::builder()
        .default_format()
        .filter_level(LevelFilter::Debug)
        .parse_default_env()
        .init();
}
