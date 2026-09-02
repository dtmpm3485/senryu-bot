use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

pub struct Metrics {
    started: Instant,
    ready: AtomicBool,
    messages: AtomicU64,
    detected: AtomicU64,
    commands: AtomicU64,
    errors: AtomicU64,
    auto_mutes: AtomicU64,
    guilds: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            ready: AtomicBool::new(false),
            messages: AtomicU64::new(0),
            detected: AtomicU64::new(0),
            commands: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            auto_mutes: AtomicU64::new(0),
            guilds: AtomicU64::new(0),
        }
    }

    pub fn set_ready(&self, value: bool) { self.ready.store(value, Ordering::Relaxed); }
    pub fn ready(&self) -> bool { self.ready.load(Ordering::Relaxed) }
    pub fn message(&self) { self.messages.fetch_add(1, Ordering::Relaxed); }
    pub fn detected(&self) { self.detected.fetch_add(1, Ordering::Relaxed); }
    pub fn command(&self) { self.commands.fetch_add(1, Ordering::Relaxed); }
    pub fn error(&self) { self.errors.fetch_add(1, Ordering::Relaxed); }
    pub fn auto_mute(&self) { self.auto_mutes.fetch_add(1, Ordering::Relaxed); }
    pub fn set_guilds(&self, n: usize) { self.guilds.store(n as u64, Ordering::Relaxed); }

    pub fn uptime_seconds(&self) -> u64 { self.started.elapsed().as_secs() }

    pub fn prometheus(&self) -> String {
        format!(
            concat!(
                "# TYPE senryu_bot_ready gauge\n",
                "senryu_bot_ready {}\n",
                "# TYPE senryu_bot_uptime_seconds gauge\n",
                "senryu_bot_uptime_seconds {}\n",
                "# TYPE senryu_bot_connected_guilds gauge\n",
                "senryu_bot_connected_guilds {}\n",
                "# TYPE senryu_bot_messages_total counter\n",
                "senryu_bot_messages_total {}\n",
                "# TYPE senryu_bot_detected_total counter\n",
                "senryu_bot_detected_total {}\n",
                "# TYPE senryu_bot_commands_total counter\n",
                "senryu_bot_commands_total {}\n",
                "# TYPE senryu_bot_errors_total counter\n",
                "senryu_bot_errors_total {}\n",
                "# TYPE senryu_bot_auto_mutes_total counter\n",
                "senryu_bot_auto_mutes_total {}\n"
            ),
            u8::from(self.ready()),
            self.uptime_seconds(),
            self.guilds.load(Ordering::Relaxed),
            self.messages.load(Ordering::Relaxed),
            self.detected.load(Ordering::Relaxed),
            self.commands.load(Ordering::Relaxed),
            self.errors.load(Ordering::Relaxed),
            self.auto_mutes.load(Ordering::Relaxed),
        )
    }
}
