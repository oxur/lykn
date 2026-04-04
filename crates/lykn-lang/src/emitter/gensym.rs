/// Deterministic gensym counter for the emitter.
///
/// Separate from the reader's gensym to ensure reproducible output
/// that matches the JS compiler's naming scheme.
#[derive(Debug)]
pub struct EmitterGensym {
    counter: usize,
}

impl EmitterGensym {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Generate the next unique name with the given prefix.
    ///
    /// Produces names of the form `{prefix}__gensym{N}`.
    pub fn next(&mut self, prefix: &str) -> String {
        let name = format!("{prefix}__gensym{}", self.counter);
        self.counter += 1;
        name
    }

    /// Reset the counter to zero.
    pub fn reset(&mut self) {
        self.counter = 0;
    }
}

impl Default for EmitterGensym {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_produces_sequential_names() {
        let mut g = EmitterGensym::new();
        assert_eq!(g.next("t"), "t__gensym0");
        assert_eq!(g.next("t"), "t__gensym1");
        assert_eq!(g.next("result"), "result__gensym2");
    }

    #[test]
    fn test_reset_restarts_counter() {
        let mut g = EmitterGensym::new();
        g.next("x");
        g.next("x");
        g.reset();
        assert_eq!(g.next("x"), "x__gensym0");
    }

    #[test]
    fn test_default_starts_at_zero() {
        let mut g = EmitterGensym::default();
        assert_eq!(g.next("v"), "v__gensym0");
    }
}
