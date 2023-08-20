use log::{Level, Metadata, Record};

static mut LOGGER: ConsoleLogger = ConsoleLogger {
    level: Level::Error,
};

pub struct ConsoleLogger {
    level: Level,
}

pub fn install(level: Level) -> &'static ConsoleLogger {
    // It's safe. Everyone chill.
    unsafe {
        log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(level.to_level_filter()))
            .unwrap();
        LOGGER.set_log_level(level);
        &LOGGER
    }
}

impl ConsoleLogger {
    pub fn set_log_level(&mut self, level: Level) {
        self.level = level;
    }
}

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
