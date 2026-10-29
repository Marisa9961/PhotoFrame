use anyhow::Result;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Copy)]
pub(super) enum PreviewTrigger {
    Immediate,
    Input,
    Resize,
}

impl PreviewTrigger {
    pub fn delay(self) -> Duration {
        match self {
            Self::Immediate => Duration::ZERO,
            Self::Input => Duration::from_millis(40),
            Self::Resize => Duration::from_millis(80),
        }
    }
}

#[derive(Clone)]
pub(super) struct PreviewRequest {
    latest: Arc<AtomicU64>,
    generation: u64,
}

impl PreviewRequest {
    pub fn new(latest: Arc<AtomicU64>) -> Self {
        let generation = latest.fetch_add(1, Ordering::Relaxed) + 1;
        Self { latest, generation }
    }

    pub fn is_current(&self) -> bool {
        self.latest.load(Ordering::Relaxed) == self.generation
    }

    /// Stop at stage boundaries, including when an obsolete decode/render failed.
    /// The decoders and compositor themselves are synchronous and run to completion.
    pub fn run<T, U>(
        &self,
        load: impl FnOnce() -> Result<T>,
        compose: impl FnOnce(T) -> Result<U>,
    ) -> Result<Option<U>> {
        if !self.is_current() {
            return Ok(None);
        }
        let photo = load();
        if !self.is_current() {
            return Ok(None);
        }
        let pixels = compose(photo?);
        if !self.is_current() {
            return Ok(None);
        }
        pixels.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_is_immediate_and_continuous_events_have_short_distinct_delays() {
        assert_eq!(PreviewTrigger::Immediate.delay(), Duration::ZERO);
        assert!(PreviewTrigger::Input.delay() > Duration::ZERO);
        assert!(PreviewTrigger::Input.delay() < PreviewTrigger::Resize.delay());
        assert!(PreviewTrigger::Resize.delay() < Duration::from_millis(160));
    }

    #[test]
    fn only_the_latest_request_runs_and_publishes() {
        let latest = Arc::new(AtomicU64::new(0));
        let requests: Vec<_> = (0..20)
            .map(|_| PreviewRequest::new(latest.clone()))
            .collect();
        for request in &requests[..19] {
            assert_eq!(
                request
                    .run(|| panic!("obsolete decode"), |_: ()| Ok(1))
                    .unwrap(),
                None
            );
        }
        assert_eq!(
            requests[19].run(|| Ok(2), |photo| Ok(photo * 3)).unwrap(),
            Some(6)
        );
    }

    #[test]
    fn request_replaced_during_decode_skips_composition_and_obsolete_errors() {
        let latest = Arc::new(AtomicU64::new(0));
        let request = PreviewRequest::new(latest.clone());
        let result = request
            .run(
                || -> Result<()> {
                    let _newer = PreviewRequest::new(latest);
                    anyhow::bail!("old decode failed");
                },
                |_| -> Result<()> { panic!("obsolete composition") },
            )
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn request_replaced_during_composition_skips_output_but_current_errors_propagate() {
        let latest = Arc::new(AtomicU64::new(0));
        let request = PreviewRequest::new(latest.clone());
        assert_eq!(
            request
                .run(
                    || Ok(()),
                    |_| {
                        let _newer = PreviewRequest::new(latest.clone());
                        Ok(7)
                    }
                )
                .unwrap(),
            None
        );
        let current = PreviewRequest::new(latest);
        assert!(
            current
                .run(
                    || -> Result<()> { anyhow::bail!("decode failed") },
                    |_| Ok(())
                )
                .is_err()
        );
        assert!(
            current
                .run(
                    || Ok(()),
                    |_| -> Result<()> { anyhow::bail!("compose failed") }
                )
                .is_err()
        );
    }
}
