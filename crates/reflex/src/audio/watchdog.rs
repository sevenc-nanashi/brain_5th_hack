/// Detect a persistent buffer-boundary stall, not a normal boundary crossing.
#[derive(Default)]
pub struct StreamWatchdog {
    previous: Option<([u32; 2], u32)>,
    since_us: u64,
}

impl StreamWatchdog {
    pub const fn new() -> Self {
        Self {
            previous: None,
            since_us: 0,
        }
    }

    pub fn stalled(
        &mut self,
        cursors: [u32; 2],
        byte_position: u32,
        buffer_bytes: u32,
        now_us: u64,
        timeout_us: u64,
    ) -> bool {
        let boundary = cursors == [0, buffer_bytes] || cursors == [buffer_bytes, 0];
        let current = (cursors, byte_position);
        if !boundary || self.previous != Some(current) {
            self.since_us = now_us;
        }
        self.previous = Some(current);
        boundary && now_us.wrapping_sub(self.since_us) >= timeout_us
    }
}
