//! Shared QPC timeline for independently clocked capture devices.
use std::collections::VecDeque;

pub(super) const TICKS_PER_SECOND: u64 = 10_000_000;

pub(super) fn frame_ticks(frames: u64, rate: u32) -> u64 {
    ((u128::from(frames) * u128::from(TICKS_PER_SECOND) + u128::from(rate / 2)) / u128::from(rate))
        as u64
}

pub(super) struct Timeline {
    start: u64,
    pauses: Vec<(u64, Option<u64>)>,
    pub end: Option<u64>,
}

impl Timeline {
    pub fn new(start: u64) -> Self {
        Self {
            start,
            pauses: Vec::new(),
            end: None,
        }
    }

    pub fn pause(&mut self, now: u64) {
        if self.end.is_none() && !self.pauses.last().is_some_and(|(_, end)| end.is_none()) {
            self.pauses.push((now, None));
        }
    }

    pub fn resume(&mut self, now: u64) {
        if let Some((_, end)) = self.pauses.last_mut()
            && end.is_none()
        {
            *end = Some(now);
        }
    }

    pub fn elapsed_frames(&self, time: u64, rate: u32) -> u64 {
        let time = self.end.map_or(time, |end| time.min(end));
        let paused: u64 = self
            .pauses
            .iter()
            .map(|(start, end)| end.unwrap_or(time).min(time).saturating_sub(*start))
            .sum();
        let ticks = time.saturating_sub(self.start).saturating_sub(paused);
        ((u128::from(ticks) * u128::from(rate) + u128::from(TICKS_PER_SECOND / 2))
            / u128::from(TICKS_PER_SECOND)) as u64
    }

    fn position(&self, time: u64, rate: u32) -> Option<u64> {
        if time < self.start
            || self.end.is_some_and(|end| time >= end)
            || self
                .pauses
                .iter()
                .any(|(start, end)| time >= *start && end.is_none_or(|end| time < end))
        {
            return None;
        }
        Some(self.elapsed_frames(time, rate))
    }

    // Split only at pauses/start/stop boundaries. No per-sample timestamp allocations.
    pub fn segments(&self, start: u64, frames: usize, rate: u32) -> Vec<(usize, usize, u64)> {
        let mut result = Vec::new();
        let mut run: Option<(usize, usize, u64)> = None;
        for index in 0..frames {
            let position =
                self.position(start.saturating_add(frame_ticks(index as u64, rate)), rate);
            match (run.as_mut(), position) {
                // Samples inside an uninterrupted packet are consecutive. Rounding each
                // QPC timestamp back to frames creates alternating holes/overlaps near
                // half-frame boundaries (100 ns ticks cannot represent every sample).
                (Some((_, count, _)), Some(_)) => *count += 1,
                (_, next) => {
                    if let Some(previous) = run.take() {
                        result.push(previous);
                    }
                    run = next.map(|frame| (index, 1, frame));
                }
            }
        }
        if let Some(previous) = run {
            result.push(previous);
        }
        result
    }
}

#[derive(Default)]
pub(super) struct TimedQueue {
    packets: VecDeque<(u64, Vec<f32>)>,
    pub late_frames: u64,
    pub silent_frames: u64,
    previous_end: Option<u64>,
    pub packet_gap_frames: u64,
    pub packet_overlap_frames: u64,
    pub max_packet_gap: u64,
    pub corrected_boundaries: u64,
}

impl TimedQueue {
    #[cfg(test)]
    pub fn push(&mut self, start: u64, samples: Vec<f32>) {
        self.push_clocked(start, samples, false);
    }

    pub fn push_clocked(&mut self, mut start: u64, mut samples: Vec<f32>, continuous: bool) {
        if let Some(end) = self.previous_end {
            let gap = start.saturating_sub(end);
            self.packet_gap_frames += gap;
            self.packet_overlap_frames += end.saturating_sub(start);
            self.max_packet_gap = self.max_packet_gap.max(gap);
            // WASAPI device positions establish continuity; QPC jitter must not
            // insert zero samples or discard the beginning of a continuous packet.
            // Fit the complete packet to its measured end, keeping the shared clock
            // (no cumulative shift). Genuine discontinuities bypass this correction.
            let frames = samples.len() / 2;
            let target_end = start + frames as u64;
            let target_frames = target_end.saturating_sub(end) as usize;
            if continuous
                && start != end
                // At 44.1 kHz, 110 frames are about 2.5 ms. Also cap the
                // adjustment to a quarter packet to avoid stretching short tails.
                && start.abs_diff(end) <= 110.min((frames / 4) as u64)
                && frames > 1
                && target_frames > 1
            {
                let mut fitted = Vec::with_capacity(target_frames * 2);
                for index in 0..target_frames {
                    let position = index as f64 * (frames - 1) as f64 / (target_frames - 1) as f64;
                    let left = position.floor() as usize;
                    let right = (left + 1).min(frames - 1);
                    let fraction = (position - left as f64) as f32;
                    for channel in 0..2 {
                        fitted.push(
                            samples[left * 2 + channel] * (1.0 - fraction)
                                + samples[right * 2 + channel] * fraction,
                        );
                    }
                }
                samples = fitted;
                start = end;
                self.corrected_boundaries += 1;
            }
        }
        self.previous_end = Some(start + (samples.len() / 2) as u64);
        self.packets.push_back((start, samples));
    }

    pub fn read(&mut self, cursor: u64, frames: usize) -> Vec<f32> {
        let end = cursor + frames as u64;
        let mut result = vec![0.0; frames * 2];
        let mut covered_until = cursor;
        let mut covered = 0;
        while let Some((start, samples)) = self.packets.front() {
            let packet_end = *start + (samples.len() / 2) as u64;
            if *start >= end {
                break;
            }
            let first = (*start).max(covered_until);
            let last = packet_end.min(end);
            if last > first {
                let source = ((first - *start) * 2) as usize;
                let target = ((first - cursor) * 2) as usize;
                let count = ((last - first) * 2) as usize;
                result[target..target + count].copy_from_slice(&samples[source..source + count]);
                covered += last - first;
                covered_until = last;
            }
            if packet_end <= end {
                self.late_frames += cursor
                    .saturating_sub(*start)
                    .min((samples.len() / 2) as u64);
                self.packets.pop_front();
            } else {
                // Retain only the unwritten tail, so overlaps and late counts remain exact.
                if let Some((start, samples)) = self.packets.front_mut() {
                    let consumed = ((end - *start) * 2) as usize;
                    samples.drain(..consumed);
                    *start = end;
                }
                break;
            }
        }
        self.silent_frames += frames as u64 - covered;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_clock_jitter_preserves_audio_and_absolute_end() {
        let mut queue = TimedQueue::default();
        queue.push_clocked(0, vec![0.5; 882], false);
        queue.push_clocked(471, vec![0.5; 882], true);
        queue.push_clocked(882, vec![0.5; 882], true);
        assert_eq!(queue.read(0, 1323), vec![0.5; 2646]);
        assert_eq!(queue.silent_frames, 0);
        assert_eq!(queue.previous_end, Some(1323));
        assert_eq!(queue.corrected_boundaries, 2);
    }

    #[test]
    fn observed_94_frame_jitter_is_corrected_without_shifting_the_end() {
        let mut queue = TimedQueue::default();
        queue.push_clocked(0, vec![0.5; 882], false);
        queue.push_clocked(535, vec![0.5; 882], true);
        queue.push_clocked(882, vec![0.5; 882], true);
        assert_eq!(queue.read(0, 1323), vec![0.5; 2646]);
        assert_eq!(queue.silent_frames, 0);
        assert_eq!(queue.previous_end, Some(1323));
        assert_eq!(queue.corrected_boundaries, 2);
    }

    #[test]
    fn large_gaps_and_short_tails_are_not_stretched() {
        for (start, frames) in [(600, 441), (451, 8)] {
            let mut queue = TimedQueue::default();
            queue.push_clocked(0, vec![0.5; 882], false);
            queue.push_clocked(start, vec![0.5; frames * 2], true);
            assert_eq!(queue.corrected_boundaries, 0);
        }
    }

    #[test]
    fn actual_discontinuity_is_not_stretched_away() {
        let mut queue = TimedQueue::default();
        queue.push_clocked(0, vec![0.5; 882], false);
        queue.push_clocked(471, vec![0.5; 882], false);
        queue.read(0, 912);
        assert_eq!(queue.silent_frames, 30);
        assert_eq!(queue.corrected_boundaries, 0);
    }

    #[test]
    fn fractional_packet_start_preserves_every_sample() {
        let timeline = Timeline::new(0);
        for start in 10000..10227 {
            let segments = timeline.segments(start, 441, 44100);
            assert_eq!(
                segments,
                vec![(0, 441, timeline.elapsed_frames(start, 44100))],
                "continuous packet at QPC {start} must not gain holes or overlaps"
            );
        }
    }

    #[test]
    fn late_start_and_missing_system_audio_keep_their_positions() {
        let mut mic = TimedQueue::default();
        let mut system = TimedQueue::default();
        mic.push(0, vec![1.0; 20]);
        system.push(3, vec![2.0; 4]);
        system.push(8, vec![3.0; 4]);
        assert_eq!(mic.read(0, 10), vec![1.0; 20]);
        assert_eq!(
            system.read(0, 10),
            [vec![0.0; 6], vec![2.0; 4], vec![0.0; 6], vec![3.0; 4]].concat()
        );
    }

    #[test]
    fn late_packets_do_not_shift_future_audio() {
        let mut queue = TimedQueue::default();
        assert_eq!(queue.read(0, 10), vec![0.0; 20]);
        queue.push(5, vec![1.0; 20]);
        assert_eq!(queue.read(10, 10), [vec![1.0; 10], vec![0.0; 10]].concat());
        assert_eq!(queue.late_frames, 5);
    }

    #[test]
    fn tails_survive_chunk_boundaries() {
        let mut queue = TimedQueue::default();
        queue.push(0, vec![0.5; 14]);
        assert_eq!(queue.read(0, 4), vec![0.5; 8]);
        assert_eq!(queue.read(4, 3), vec![0.5; 6]);
        assert_eq!(queue.late_frames, 0);
    }

    #[test]
    fn pauses_remove_only_paused_samples_even_from_buffered_packets() {
        let mut timeline = Timeline::new(0);
        timeline.pause(2 * TICKS_PER_SECOND);
        timeline.resume(4 * TICKS_PER_SECOND);
        assert_eq!(
            timeline.segments(TICKS_PER_SECOND, 4, 1),
            vec![(0, 1, 1), (3, 1, 2)]
        );
        assert_eq!(timeline.elapsed_frames(5 * TICKS_PER_SECOND, 1), 3);
        timeline.end = Some(5 * TICKS_PER_SECOND);
        assert_eq!(
            timeline.segments(4 * TICKS_PER_SECOND, 3, 1),
            vec![(0, 1, 2)]
        );
        assert_eq!(timeline.elapsed_frames(20 * TICKS_PER_SECOND, 1), 3);
    }

    #[test]
    fn fractional_sample_timestamps_do_not_accumulate_drift() {
        let timeline = Timeline::new(TICKS_PER_SECOND);
        let start = 600 * 44100;
        assert_eq!(
            timeline.segments(TICKS_PER_SECOND + frame_ticks(start, 44100), 44100, 44100),
            vec![(0, 44100, start)]
        );
    }

    #[test]
    fn packets_before_start_are_trimmed() {
        let timeline = Timeline::new(2 * TICKS_PER_SECOND);
        assert_eq!(timeline.segments(TICKS_PER_SECOND, 3, 1), vec![(1, 2, 0)]);
    }
}
