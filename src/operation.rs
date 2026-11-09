//! Cooperative cancellation extends across blocking index work and transaction commits.
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
thread_local! {static CURRENT:RefCell<Option<(CancellationToken,Instant)>>=const{RefCell::new(None)};}
pub fn scoped<T>(token: CancellationToken, f: impl FnOnce() -> T) -> T {
    CURRENT.with(|c| *c.borrow_mut() = Some((token, Instant::now())));
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT.with(|c| *c.borrow_mut() = None);
        }
    }
    let _reset = Reset;
    f()
}
pub fn check() -> anyhow::Result<()> {
    CURRENT.with(|c| {
        if let Some((token, started)) = c.borrow().as_ref() {
            anyhow::ensure!(
                !token.is_cancelled(),
                "operation cancelled; pending ledger transaction will not commit. Earlier bootstrap/artifact files may exist; inspect before retrying"
            );
            anyhow::ensure!(
                started.elapsed() < Duration::from_secs(45),
                "operation deadline exceeded; pending ledger transaction will not commit. Earlier bootstrap/artifact files may exist; inspect before retrying"
            );
        }
        Ok(())
    })
}
