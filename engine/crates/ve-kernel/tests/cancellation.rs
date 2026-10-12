//! G2 cooperative cancellation uses the real Manifold context; worker termination is separate.
use ve_kernel::{ExecutionContext, KernelError, Solid};

#[test]
fn g2_08_cancelled_boolean_returns_no_cacheable_solid() {
    let a = Solid::cube([1.0; 3], true).unwrap();
    let b = Solid::sphere(0.6, 32).unwrap();
    let context = ExecutionContext::new().unwrap();
    context.cancel();
    assert!(context.is_cancelled());
    assert!(matches!(
        a.union_with_context(&b, &context),
        Err(KernelError::Cancelled)
    ));
    let fresh = ExecutionContext::new().unwrap();
    assert!(a.union_with_context(&b, &fresh).unwrap().volume() > 1.0);
    assert!(!fresh.is_cancelled());
}

#[test]
fn g2_08_cancel_during_native_boolean_tree() {
    use std::sync::{Arc, Barrier};
    let context = Arc::new(ExecutionContext::new().unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let observer = Arc::clone(&context);
    let signal = Arc::clone(&barrier);
    let task = std::thread::spawn(move || {
        let solids: Vec<_> = (0..32)
            .map(|i| {
                Solid::sphere(1.0, 96)
                    .unwrap()
                    .translate([f64::from(i) * 0.1, 0.0, 0.0])
                    .unwrap()
            })
            .collect();
        let refs: Vec<_> = solids.iter().collect();
        signal.wait();
        Solid::union_all_with_context(&refs, &observer)
    });
    barrier.wait();
    std::thread::sleep(std::time::Duration::from_millis(2));
    context.cancel();
    assert!(matches!(task.join().unwrap(), Err(KernelError::Cancelled)));
}
