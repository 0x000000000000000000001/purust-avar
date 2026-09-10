use purust_core::{Func1, Value};
use std::collections::HashSet;
use std::sync::{mpsc, Arc, Barrier, Mutex};
use std::time::Duration;
use Purs_Data_Either::Either;
use Purs_Effect_AVar::*;

fn run(action: Value) -> Value {
    action.unwrap_func1()(Value::Unit)
}

fn noop() -> Value {
    Value::Func1(Func1::Static(|_| Value::Unit))
}

fn new_var(value: Option<Value>) -> Arc<AVar> {
    let action = match value {
        Some(value) => Effect_AVar__newVar(value),
        None => Effect_AVar_empty(),
    };
    run(action).unwrap_class::<Arc<AVar>>().clone()
}

#[test]
fn canceled_waiters_release_callbacks_and_values() {
    for queue in 0..3 {
        let var = new_var(if queue == 0 { Some(Value::Unit) } else { None });
        let marker = Arc::new(());
        let callback_weak = Arc::downgrade(&marker);
        let callback = Func1::Shared(Arc::new(move |_: Arc<Either>| {
            std::hint::black_box(&marker);
            noop()
        }));
        let value = Arc::new(());
        let value_weak = Arc::downgrade(&value);
        let action = match queue {
            0 => Effect_AVar_put(Value::Class(value), var, callback),
            1 => {
                drop(value);
                Effect_AVar_read(var, callback)
            }
            _ => {
                drop(value);
                Effect_AVar_take(var, callback)
            }
        };
        let cancel = run(action);
        assert!(callback_weak.upgrade().is_some());
        if queue == 0 {
            assert!(value_weak.upgrade().is_some());
        }
        run(cancel.clone());
        assert!(
            callback_weak.upgrade().is_none(),
            "canceled callback retained"
        );
        assert!(
            value_weak.upgrade().is_none(),
            "canceled put value retained"
        );
        run(cancel);
    }
}

#[test]
fn concurrent_producers_and_consumers_deliver_each_value_once() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(15));
        eprintln!("AVar native concurrency test timed out");
        std::process::exit(124);
    });
    const WORKERS: usize = 4;
    const EACH: usize = 250;
    let var = new_var(None);
    let gate = Arc::new(Barrier::new(WORKERS * 2));
    let values = Arc::new(Mutex::new(Vec::new()));
    let worker_ids = Arc::new(Mutex::new(HashSet::new()));
    let mut workers = Vec::new();
    for _ in 0..WORKERS {
        let var = var.clone();
        let gate = gate.clone();
        let values = values.clone();
        let worker_ids = worker_ids.clone();
        workers.push(std::thread::spawn(move || {
            worker_ids
                .lock()
                .unwrap()
                .insert(std::thread::current().id());
            gate.wait();
            for _ in 0..EACH {
                let (sender, receiver) = mpsc::channel();
                let reentrant = var.clone();
                let callback = Func1::Shared(Arc::new(move |result: Arc<Either>| {
                    let sender = sender.clone();
                    let reentrant = reentrant.clone();
                    Value::Func1(Func1::Shared(Arc::new(move |_| {
                        // Taking the lock again proves user callbacks run outside it.
                        run(Effect_AVar_status(reentrant.clone()));
                        match result.as_ref() {
                            Either::Right(value) => sender.send(value.unwrap_int()).unwrap(),
                            Either::Left(_) => panic!("unexpected killed AVar"),
                        }
                        Value::Unit
                    })))
                }));
                let _cancel = run(Effect_AVar_take(var.clone(), callback));
                let value = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
                values.lock().unwrap().push(value);
            }
        }));
    }
    for worker in 0..WORKERS {
        let var = var.clone();
        let gate = gate.clone();
        let worker_ids = worker_ids.clone();
        workers.push(std::thread::spawn(move || {
            worker_ids
                .lock()
                .unwrap()
                .insert(std::thread::current().id());
            gate.wait();
            for index in 0..EACH {
                let (sender, receiver) = mpsc::channel();
                let callback = Func1::Shared(Arc::new(move |result: Arc<Either>| {
                    let sender = sender.clone();
                    Value::Func1(Func1::Shared(Arc::new(move |_| {
                        assert!(matches!(result.as_ref(), Either::Right(Value::Unit)));
                        sender.send(()).unwrap();
                        Value::Unit
                    })))
                }));
                let value = Value::Int((worker * EACH + index) as i64);
                let _cancel = run(Effect_AVar_put(value, var.clone(), callback));
                receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(worker_ids.lock().unwrap().len(), WORKERS * 2);
    let mut values = values.lock().unwrap().clone();
    values.sort_unstable();
    assert_eq!(values, (0..(WORKERS * EACH) as i64).collect::<Vec<_>>());
}
