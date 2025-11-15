#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Hold,
    Linear,
}

#[derive(Clone, Copy, Debug)]
pub struct Keyframe {
    pub time: usize,
    pub value: isize,
    pub interpolation: Interpolation,
}

impl Keyframe {
    pub fn new(time: usize, value: isize, interpolation: Interpolation) -> Self {
        Self {
            time,
            value,
            interpolation,
        }
    }
}

/// An animation curve.
///
/// This is a function of time defined by [Keyframe]s. The curve takes on a keyframe's value
/// wherever one is present, and otherwise interpolates between the two nearest keyframes using the
/// earlier keyframe's interpolation method (an [Interpolation] variant). At any time before the
/// first keyframe the curve's value is zero, and at any time after the last keyframe it is that
/// keyframe's value.
#[derive(Default)]
pub struct Curve {
    keyframes: Vec<Keyframe>,
    dirty: usize,
}

impl Curve {
    pub fn keyframe(&self, time: usize) -> Option<Keyframe> {
        Some(self.keyframes[self.keyframe_index(time).ok()?])
    }

    pub fn keyframe_mut(&mut self, time: usize) -> Option<&mut Keyframe> {
        self.mark_dirty(time);
        let index = self.keyframe_index(time).ok()?;
        Some(&mut self.keyframes[index])
    }

    pub fn first_keyframe(&self) -> Option<Keyframe> {
        self.keyframes.first().cloned()
    }

    pub fn last_keyframe(&self) -> Option<Keyframe> {
        self.keyframes.last().cloned()
    }

    pub fn prev_keyframe(&self, time: usize) -> Option<Keyframe> {
        let i = match self.keyframe_index(time) {
            Ok(i) => i,
            Err(i) => i,
        };
        (i > 0).then(|| self.keyframes[i - 1])
    }

    pub fn next_keyframe(&self, time: usize) -> Option<Keyframe> {
        let i = match self.keyframe_index(time) {
            Ok(i) => i + 1,
            Err(i) => i,
        };
        (i < self.keyframes.len()).then(|| self.keyframes[i])
    }

    pub fn insert_keyframe(&mut self, keyframe: Keyframe) {
        self.mark_dirty(keyframe.time);
        match self.keyframe_index(keyframe.time) {
            Ok(i) => self.keyframes[i] = keyframe,
            Err(i) => self.keyframes.insert(i, keyframe),
        };
    }

    pub fn remove_keyframe(&mut self, time: usize) -> Option<Keyframe> {
        let result = Some(self.keyframes.remove(self.keyframe_index(time).ok()?));
        self.mark_dirty(time);
        result
    }

    pub fn keyframes_affecting_range(
        &self,
        range: impl Into<std::ops::RangeInclusive<usize>>,
    ) -> impl Iterator<Item = &Keyframe> {
        let range: std::ops::RangeInclusive<usize> = range.into();

        let start = self
            .keyframes
            .partition_point(|k| k.time < *range.start())
            .max(1)
            - 1;

        let end = (self.keyframes.partition_point(|k| k.time < *range.end()) + 1)
            .min(self.keyframes.len());

        self.keyframes[start..end].iter()
    }

    pub fn eval(&self, time: usize) -> isize {
        if self.first_keyframe().is_none_or(|k| time < k.time) {
            return 0;
        }

        if let Some(last) = self.last_keyframe()
            && time >= last.time
        {
            return last.value;
        }

        if let Some(k) = self.keyframe(time) {
            return k.value;
        }

        let (a, b) = (
            self.prev_keyframe(time).unwrap(),
            self.next_keyframe(time).unwrap(),
        );

        match a.interpolation {
            Interpolation::Hold => a.value,
            Interpolation::Linear => {
                let t = (time - a.time) as isize;
                let dt = (b.time - a.time) as isize;
                a.value + ((b.value - a.value) * t + dt / 2) / dt
            }
        }
    }

    pub fn eval_smooth(&self, time: f32) -> f32 {
        if self
            .first_keyframe()
            .is_none_or(|k| (time as usize) < k.time)
        {
            return 0.0;
        }

        if let Some(last) = self.last_keyframe()
            && (time as usize) >= last.time
        {
            return last.value as f32;
        }

        if let Some(k) = self.keyframe(time as usize) {
            return k.value as f32;
        }

        let (a, b) = (
            self.prev_keyframe(time as usize).unwrap(),
            self.next_keyframe(time as usize).unwrap(),
        );

        match a.interpolation {
            Interpolation::Hold => a.value as f32,
            Interpolation::Linear => {
                let t = (time - a.time as f32) / (b.time - a.time) as f32;
                (1.0 - t) * a.value as f32 + t * b.value as f32
            }
        }
    }

    pub fn optimize(&mut self) {
        let end_t = self.keyframes.last().unwrap().time;
        for t in 0..=end_t {
            if let (Some(prev_keyframe), Some(keyframe)) = (self.prev_keyframe(t), self.keyframe(t))
                && prev_keyframe.interpolation == Interpolation::Hold
                && keyframe.interpolation == Interpolation::Hold
                && prev_keyframe.value == keyframe.value
            {
                self.remove_keyframe(t);
            }
        }
    }

    pub fn dirty(&self) -> usize {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = usize::MAX;
    }

    fn mark_dirty(&mut self, time: usize) {
        let dirty_time = match self.prev_keyframe(time) {
            Some(Keyframe {
                time: prev_time,
                interpolation: Interpolation::Linear,
                ..
            }) => prev_time + 1,
            _ => time,
        };
        self.dirty = self.dirty.min(dirty_time);
    }

    fn keyframe_index(&self, time: usize) -> Result<usize, usize> {
        self.keyframes.binary_search_by(|k| k.time.cmp(&time))
    }
}
