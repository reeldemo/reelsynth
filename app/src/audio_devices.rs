//! Audio output device enumeration and hot-plug diff helpers.

use cpal::traits::{DeviceTrait, HostTrait};

/// Names of available CPAL output devices (order matches host enumeration).
#[derive(Debug, Clone, Default)]
pub struct AudioOutputDevices {
    pub names: Vec<String>,
}

impl AudioOutputDevices {
    pub fn enumerate() -> Self {
        let host = cpal::default_host();
        let mut names = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for device in devices {
                if let Ok(name) = device.name() {
                    names.push(name);
                }
            }
        }
        Self { names }
    }

    /// Re-enumerate; returns `true` when the ordered name list changed.
    pub fn refresh(&mut self) -> bool {
        let fresh = Self::enumerate();
        let changed = fresh.names != self.names;
        *self = fresh;
        changed
    }

    pub fn index_of_name(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }

    /// Host default output name when present in the enumerated list.
    pub fn default_name() -> Option<String> {
        let host = cpal::default_host();
        host.default_output_device()
            .and_then(|d| d.name().ok())
    }
}

/// Names present in `current` but not in `previous`, preserving `current` order.
///
/// Used to pick freshly connected outputs without thrashing on every default-device poll.
pub fn newly_appeared(previous: &[String], current: &[String]) -> Vec<String> {
    current
        .iter()
        .filter(|name| !previous.iter().any(|p| p == *name))
        .cloned()
        .collect()
}

/// Prefer the last newly appeared device (most recently connected in enum order).
pub fn prefer_fresh_device(previous: &[String], current: &[String]) -> Option<String> {
    newly_appeared(previous, current).into_iter().last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newly_appeared_empty_when_unchanged() {
        let list = vec!["Speakers".into(), "Headphones".into()];
        assert!(newly_appeared(&list, &list).is_empty());
        assert_eq!(prefer_fresh_device(&list, &list), None);
    }

    #[test]
    fn newly_appeared_detects_additions_only() {
        let previous = vec!["Speakers".into()];
        let current = vec![
            "Speakers".into(),
            "Focusrite USB".into(),
            "DI Box".into(),
        ];
        assert_eq!(
            newly_appeared(&previous, &current),
            vec!["Focusrite USB".to_string(), "DI Box".to_string()]
        );
        assert_eq!(
            prefer_fresh_device(&previous, &current),
            Some("DI Box".into())
        );
    }

    #[test]
    fn newly_appeared_ignores_removals_and_reorders() {
        let previous = vec!["A".into(), "B".into(), "C".into()];
        let current = vec!["C".into(), "A".into()];
        assert!(newly_appeared(&previous, &current).is_empty());
    }

    #[test]
    fn newly_appeared_from_empty_baseline() {
        let current = vec!["Only Device".into()];
        assert_eq!(
            prefer_fresh_device(&[], &current),
            Some("Only Device".into())
        );
    }
}
