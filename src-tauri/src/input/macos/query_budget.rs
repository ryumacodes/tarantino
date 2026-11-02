//! Bound the aggregate work of a caret lookup across related/descendant elements.
use std::{
    cell::Cell,
    time::{Duration, Instant},
};
thread_local! { static DEADLINE:Cell<Option<Instant>>=const {Cell::new(None)}; }

pub(super) struct LookupBudget(Option<Instant>);
impl LookupBudget {
    pub(super) fn begin() -> Self {
        Self(
            DEADLINE.with(|deadline| {
                deadline.replace(Some(Instant::now() + Duration::from_millis(50)))
            }),
        )
    }
}
impl Drop for LookupBudget {
    fn drop(&mut self) {
        DEADLINE.with(|deadline| deadline.set(self.0));
    }
}
pub(super) fn available() -> bool {
    DEADLINE.with(|deadline| deadline.get().is_none_or(|end| Instant::now() < end))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_lookup_skips_further_requests_and_scope_restores_budget() {
        assert!(available());
        {
            let _scope = LookupBudget::begin();
            DEADLINE.with(|deadline| deadline.set(Some(Instant::now() - Duration::from_millis(1))));
            assert!(!available());
            {
                let _nested = LookupBudget::begin();
                assert!(available());
            }
            assert!(!available());
        }
        assert!(available());
    }
}
