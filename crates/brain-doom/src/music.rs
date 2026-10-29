//! MUS sequencing and GENMIDI instruments; integer PCM synthesis via Opal.
use core::ptr;

const BANK: &[u8] = include_bytes!(env!("DOOM_GENMIDI"));
const PITCH: [u16; 4096] = include!(concat!(env!("OUT_DIR"), "/opl-pitch.rs"));
const LEVEL: [u8; 128] = include!(concat!(env!("OUT_DIR"), "/opl-volume.rs"));
const OPERATORS: [u16; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];
unsafe extern "C" {
    fn brain_opl_reset();
    fn brain_opl_write(reg: u16, value: u8);
    fn brain_opl_sample(out: *mut i16);
}

#[derive(Clone, Copy)]
struct Channel {
    program: usize,
    velocity: u8,
    volume: u8,
    expression: u8,
    pan: u8,
    bend: i32,
    sustain: bool,
}
impl Channel {
    const NEW: Self = Self {
        program: 0,
        velocity: 127,
        volume: 100,
        expression: 127,
        pan: 64,
        bend: 0,
        sustain: false,
    };
}
#[derive(Clone, Copy)]
struct Voice {
    channel: usize,
    key: u8,
    velocity: u8,
    instrument: usize,
    part: usize,
    on: bool,
    held: bool,
    age: u64,
}
impl Voice {
    const NEW: Self = Self {
        channel: 0,
        key: 0,
        velocity: 0,
        instrument: 0,
        part: 0,
        on: false,
        held: false,
        age: 0,
    };
}
struct Music {
    song: usize,
    cursor: usize,
    wait: u32,
    phase: u32,
    ended: bool,
    age: u64,
    channels: [Channel; 16],
    voices: [Voice; 18],
}
impl Music {
    const NEW: Self = Self {
        song: 0,
        cursor: 0,
        wait: 0,
        phase: 11025,
        ended: false,
        age: 0,
        channels: [Channel::NEW; 16],
        voices: [Voice::NEW; 18],
    };

    fn write(reg: u16, value: u8) {
        unsafe { brain_opl_write(reg, value) }
    }
    fn reg(index: usize) -> u16 {
        (index % 9) as u16 | if index >= 9 { 0x100 } else { 0 }
    }
    fn operator(index: usize) -> u16 {
        OPERATORS[index % 9] | if index >= 9 { 0x100 } else { 0 }
    }

    fn frequency(&self, index: usize) {
        let v = self.voices[index];
        let instrument = &BANK[8 + v.instrument * 36..][..36];
        let fixed = instrument[0] & 1 != 0;
        let offset = 4 + v.part * 16;
        let mut note = if fixed {
            instrument[3] as i32
        } else {
            v.key as i32
                + i16::from_le_bytes([instrument[offset + 14], instrument[offset + 15]]) as i32
        };
        while note < 0 {
            note += 12;
        }
        while note > 95 {
            note -= 12;
        }
        let detune = if v.part == 1 {
            instrument[2] as i32 / 2 - 64
        } else {
            0
        };
        let pitch = (note * 32 + self.channels[v.channel].bend + detune).clamp(0, 4095);
        let frequency = PITCH[pitch as usize];
        Self::write(0xa0 + Self::reg(index), frequency as u8);
        Self::write(
            0xb0 + Self::reg(index),
            (frequency >> 8) as u8 | if v.on { 0x20 } else { 0 },
        );
    }

    fn volume(&self, index: usize) {
        let v = self.voices[index];
        let c = self.channels[v.channel];
        let data = &BANK[8 + v.instrument * 36 + 4 + v.part * 16..][..16];
        let level = v.velocity as u32 * c.volume as u32 * c.expression as u32 / (127 * 127);
        let attenuation = LEVEL[level as usize];
        let op = Self::operator(index);
        for (offset, register, audible) in [(0, op, data[6] & 1 != 0), (7, op + 3, true)] {
            let base = data[offset + 5] & 63;
            let level = if audible {
                base.saturating_add(attenuation).min(63)
            } else {
                base
            };
            Self::write(0x40 + register, (data[offset + 4] & 0xc0) | level);
        }
        let pan = if c.pan < 32 {
            0x10
        } else if c.pan > 95 {
            0x20
        } else {
            0x30
        };
        Self::write(0xc0 + Self::reg(index), data[6] | pan);
    }

    fn release(&mut self, index: usize) {
        self.voices[index].held = false;
        self.voices[index].on = false;
        self.frequency(index);
    }

    fn note_off(&mut self, channel: usize, key: u8) {
        for i in 0..18 {
            if self.voices[i].on && self.voices[i].channel == channel && self.voices[i].key == key {
                self.voices[i].held = false;
                if !self.channels[channel].sustain {
                    self.release(i);
                }
            }
        }
    }

    fn note_on(&mut self, channel: usize, key: u8) {
        let instrument = if channel == 15 {
            if !(35..=81).contains(&key) {
                return;
            }
            128 + key as usize - 35
        } else {
            self.channels[channel].program
        };
        if self.channels[channel].velocity == 0 {
            self.note_off(channel, key);
            return;
        }
        let parts = if BANK[8 + instrument * 36] & 4 != 0 {
            2
        } else {
            1
        };
        for part in 0..parts {
            let index = (0..18)
                .min_by_key(|&i| (self.voices[i].on, self.voices[i].age))
                .unwrap();
            self.release(index);
            self.age += 1;
            self.voices[index] = Voice {
                channel,
                key,
                velocity: self.channels[channel].velocity,
                instrument,
                part,
                on: true,
                held: true,
                age: self.age,
            };
            let data = &BANK[8 + instrument * 36 + 4 + part * 16..][..16];
            for (offset, op) in [(0, Self::operator(index)), (7, Self::operator(index) + 3)] {
                Self::write(0x20 + op, data[offset]);
                Self::write(0x60 + op, data[offset + 1]);
                Self::write(0x80 + op, data[offset + 2]);
                Self::write(0xe0 + op, data[offset + 3]);
            }
            self.volume(index);
            self.frequency(index);
        }
    }

    fn control(&mut self, channel: usize, control: u8, value: u8) {
        let c = &mut self.channels[channel];
        match control {
            0 => c.program = value as usize,
            3 => c.volume = value,
            4 => c.pan = value,
            5 => c.expression = value,
            8 => c.sustain = value >= 64,
            14 => {
                c.expression = 127;
                c.bend = 0;
                c.sustain = false;
            }
            _ => {}
        }
        for i in 0..18 {
            if self.voices[i].on && self.voices[i].channel == channel {
                if matches!(control, 10 | 11)
                    || (!self.channels[channel].sustain && !self.voices[i].held)
                {
                    self.release(i);
                } else {
                    self.volume(i);
                    self.frequency(i);
                }
            }
        }
    }

    fn byte(&mut self, score: &[u8]) -> Option<u8> {
        let value = *score.get(self.cursor)?;
        self.cursor += 1;
        Some(value)
    }

    fn tick(&mut self, score: &[u8], looping: bool) -> Option<()> {
        if self.ended {
            return Some(());
        }
        if self.wait > 0 {
            self.wait -= 1;
        }
        if self.wait > 0 {
            return Some(());
        }
        // Bound zero-delay groups so a malformed score cannot hang the host.
        for _ in 0..score.len() {
            let event = self.byte(score)?;
            let channel = (event & 15) as usize;
            match (event >> 4) & 7 {
                0 => {
                    let note = self.byte(score)? & 127;
                    self.note_off(channel, note);
                }
                1 => {
                    let note = self.byte(score)?;
                    if note & 128 != 0 {
                        self.channels[channel].velocity = self.byte(score)? & 127;
                    }
                    self.note_on(channel, note & 127);
                }
                2 => {
                    self.channels[channel].bend = (self.byte(score)? as i32 - 128) / 2;
                    for i in 0..18 {
                        if self.voices[i].on && self.voices[i].channel == channel {
                            self.frequency(i);
                        }
                    }
                }
                3 => {
                    let control = self.byte(score)?;
                    self.control(channel, control, 0);
                }
                4 => {
                    let control = self.byte(score)?;
                    let value = self.byte(score)? & 127;
                    self.control(channel, control, value);
                }
                5 => {}
                6 => {
                    for i in 0..18 {
                        self.release(i);
                    }
                    if looping {
                        self.cursor = 0;
                    } else {
                        self.ended = true;
                        return Some(());
                    }
                    continue;
                }
                _ => return None,
            }
            if event & 128 != 0 {
                let mut delay = 0u32;
                loop {
                    let byte = self.byte(score)?;
                    delay = delay.checked_mul(128)?.checked_add((byte & 127) as u32)?;
                    if byte & 128 == 0 {
                        break;
                    }
                }
                self.wait = delay;
                if delay > 0 {
                    return Some(());
                }
            }
        }
        None
    }
}

static mut MUSIC: Music = Music::NEW;

// Called only from the engine's serialized mix checkpoint; the score belongs to its WAD cache.
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_music_mix(
    output: *mut i16,
    song: usize,
    data: *const u8,
    length: u32,
    playing: i32,
    looping: i32,
    volume: i32,
) -> i32 {
    unsafe {
        let music = &mut *ptr::addr_of_mut!(MUSIC);
        if music.song != song {
            *music = Music { song, ..Music::NEW };
            brain_opl_reset();
        }
        if song == 0 || playing == 0 {
            return 1;
        }
        if data.is_null() || length == 0 || !(0..=15).contains(&volume) {
            return 0;
        }
        let score = core::slice::from_raw_parts(data, length as usize);
        let output = core::slice::from_raw_parts_mut(output, 512 * 2);
        for stereo in output.as_chunks_mut::<2>().0 {
            if music.phase >= 11025 {
                music.phase -= 11025;
                if music.tick(score, looping != 0).is_none() {
                    return 0;
                }
            }
            music.phase += 140;
            let mut sample = [0i16; 2];
            brain_opl_sample(sample.as_mut_ptr());
            for (out, bgm) in stereo.iter_mut().zip(sample) {
                *out = (*out as i32 + bgm as i32 * volume / 30).clamp(-32768, 32767) as i16;
            }
        }
        1
    }
}
