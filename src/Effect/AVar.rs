use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Mutex;

pub struct AVar {
    state: Mutex<AVarState>,
}

#[derive(Default)]
struct AVarState {
    value: Option<Value>,
    error: Option<Value>,
    draining: bool,
    next_id: u64,
    puts: VecDeque<Put>,
    reads: VecDeque<Waiter>,
    takes: VecDeque<Waiter>,
}

struct Waiter {
    id: u64,
    callback: Value,
}

struct Put {
    waiter: Waiter,
    value: Value,
}

#[derive(Clone, Copy)]
enum Queue {
    Put,
    Read,
    Take,
}

fn avar_effect(action: impl Fn() -> Value + 'static) -> Value {
    Value::Func1(purust_core::Func1::Shared(Rc::new(move |_| action())))
}

fn avar_unbox(value: &Value) -> Rc<AVar> {
    value.unwrap_class::<Rc<AVar>>().clone()
}

fn avar_create(value: Option<Value>) -> Value {
    Value::Class(Rc::new(Rc::new(AVar {
        state: Mutex::new(AVarState {
            value,
            ..Default::default()
        }),
    })))
}

fn avar_call(callback: Value, result: Value, failure: &mut Option<Box<dyn std::any::Any + Send>>) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        callback.unwrap_func1()(result).unwrap_func1()(Value::Unit);
    }));
    if let Err(error) = outcome {
        if failure.is_none() {
            *failure = Some(error);
        }
    }
}

// Only one drainer owns callback delivery. Every callback runs outside the
// state lock, so it can enqueue, cancel, inspect or kill this same AVar.
fn avar_drain(util: &Value, avar: &Rc<AVar>) {
    {
        let mut state = avar.state.lock().expect("AVar state poisoned");
        if state.draining {
            return;
        }
        state.draining = true;
    }
    let mut failure = None;
    loop {
        let (error, put, take, value, reads) = {
            let mut state = avar.state.lock().expect("AVar state poisoned");
            if let Some(error) = state.error.clone() {
                (Some(error), None, None, None, 0)
            } else {
                let reads = state.reads.len();
                let put = if state.value.is_none() {
                    state.puts.pop_front()
                } else {
                    None
                };
                if let Some(put) = &put {
                    state.value = Some(put.value.clone());
                }
                let value = state.value.clone();
                let take = if value.is_some() {
                    state.takes.pop_front()
                } else {
                    None
                };
                // Reserve the take before releasing the lock: another thread
                // must not consume the same value while reads are notified.
                if take.is_some() {
                    state.value = None;
                }
                (None, put, take, value, reads)
            }
        };
        if let Some(error) = error {
            let result = util.get_left().unwrap_func1()(error);
            loop {
                let callback = {
                    let mut state = avar.state.lock().expect("AVar state poisoned");
                    state
                        .puts
                        .pop_front()
                        .map(|put| put.waiter)
                        .or_else(|| state.reads.pop_front())
                        .or_else(|| state.takes.pop_front())
                };
                match callback {
                    Some(waiter) => avar_call(waiter.callback, result.clone(), &mut failure),
                    None => break,
                }
            }
        } else {
            if let Some(value) = value {
                let result = util.get_right().unwrap_func1()(value);
                for _ in 0..reads {
                    let read = avar
                        .state
                        .lock()
                        .expect("AVar state poisoned")
                        .reads
                        .pop_front();
                    match read {
                        Some(waiter) => avar_call(waiter.callback, result.clone(), &mut failure),
                        None => break,
                    }
                }
                if let Some(waiter) = take {
                    avar_call(waiter.callback, result, &mut failure);
                }
            }
            if let Some(put) = put {
                let result = util.get_right().unwrap_func1()(Value::Unit);
                avar_call(put.waiter.callback, result, &mut failure);
            }
        }
        let mut state = avar.state.lock().expect("AVar state poisoned");
        let pending = if state.error.is_some() {
            !state.puts.is_empty() || !state.reads.is_empty() || !state.takes.is_empty()
        } else if state.value.is_none() {
            !state.puts.is_empty()
        } else {
            !state.takes.is_empty() || !state.reads.is_empty()
        };
        if !pending {
            state.draining = false;
            break;
        }
    }
    if let Some(error) = failure {
        std::panic::resume_unwind(error);
    }
}

fn avar_enqueue(
    util: Value,
    value: Option<Value>,
    avar: Value,
    callback: Value,
    queue: Queue,
) -> Value {
    avar_effect(move || {
        let avar = avar_unbox(&avar);
        let id = {
            let mut state = avar.state.lock().expect("AVar state poisoned");
            let id = state.next_id;
            state.next_id = state
                .next_id
                .checked_add(1)
                .expect("AVar callback id exhausted");
            let waiter = Waiter {
                id,
                callback: callback.clone(),
            };
            match queue {
                Queue::Put => state.puts.push_back(Put {
                    waiter,
                    value: value.clone().expect("put value"),
                }),
                Queue::Read => state.reads.push_back(waiter),
                Queue::Take => state.takes.push_back(waiter),
            }
            id
        };
        avar_drain(&util, &avar);
        avar_effect(move || {
            let mut state = avar.state.lock().expect("AVar state poisoned");
            match queue {
                Queue::Put => state.puts.retain(|put| put.waiter.id != id),
                Queue::Read => state.reads.retain(|waiter| waiter.id != id),
                Queue::Take => state.takes.retain(|waiter| waiter.id != id),
            }
            Value::Unit
        })
    })
}

pub fn Effect_AVar_empty() -> UnknownType {
    avar_effect(|| avar_create(None))
}

pub fn Effect_AVar__newVar(value: UnknownType) -> UnknownType {
    avar_effect(move || avar_create(Some(value.clone())))
}

pub fn Effect_AVar__putVar() -> UnknownType {
    Value::Func4(purust_core::Func4::Static(|util, value, avar, callback| {
        avar_enqueue(util, Some(value), avar, callback, Queue::Put)
    }))
}

pub fn Effect_AVar__takeVar() -> UnknownType {
    Value::Func3(purust_core::Func3::Static(|util, avar, callback| {
        avar_enqueue(util, None, avar, callback, Queue::Take)
    }))
}

pub fn Effect_AVar__readVar() -> UnknownType {
    Value::Func3(purust_core::Func3::Static(|util, avar, callback| {
        avar_enqueue(util, None, avar, callback, Queue::Read)
    }))
}

pub fn Effect_AVar__killVar() -> UnknownType {
    Value::Func3(purust_core::Func3::Static(|util, error, avar| {
        avar_effect(move || {
            let avar = avar_unbox(&avar);
            let changed = {
                let mut state = avar.state.lock().expect("AVar state poisoned");
                if state.error.is_some() {
                    false
                } else {
                    state.error = Some(error.clone());
                    state.value = None;
                    true
                }
            };
            if changed {
                avar_drain(&util, &avar);
            }
            Value::Unit
        })
    }))
}

pub fn Effect_AVar__tryPutVar() -> UnknownType {
    Value::Func3(purust_core::Func3::Static(|util, value, avar| {
        avar_effect(move || {
            let avar = avar_unbox(&avar);
            let inserted = {
                let mut state = avar.state.lock().expect("AVar state poisoned");
                if state.value.is_none() && state.error.is_none() {
                    state.value = Some(value.clone());
                    true
                } else {
                    false
                }
            };
            if inserted {
                avar_drain(&util, &avar);
            }
            mk_bool(inserted)
        })
    }))
}

pub fn Effect_AVar__tryTakeVar() -> UnknownType {
    Value::Func2(purust_core::Func2::Static(|util, avar| {
        avar_effect(move || {
            let avar = avar_unbox(&avar);
            let value = avar.state.lock().expect("AVar state poisoned").value.take();
            match value {
                Some(value) => {
                    avar_drain(&util, &avar);
                    util.get_just().unwrap_func1()(value)
                }
                None => util.get_nothing(),
            }
        })
    }))
}

pub fn Effect_AVar__tryReadVar() -> UnknownType {
    Value::Func2(purust_core::Func2::Static(|util, avar| {
        avar_effect(move || {
            let avar = avar_unbox(&avar);
            let value = avar
                .state
                .lock()
                .expect("AVar state poisoned")
                .value
                .clone();
            match value {
                Some(value) => util.get_just().unwrap_func1()(value),
                None => util.get_nothing(),
            }
        })
    }))
}

pub fn Effect_AVar__status() -> UnknownType {
    Value::Func2(purust_core::Func2::Static(|util, avar| {
        avar_effect(move || {
            let avar = avar_unbox(&avar);
            let (error, value) = {
                let state = avar.state.lock().expect("AVar state poisoned");
                (state.error.clone(), state.value.clone())
            };
            if let Some(error) = error {
                util.get_killed().unwrap_func1()(error)
            } else if let Some(value) = value {
                util.get_filled().unwrap_func1()(value)
            } else {
                util.get_empty()
            }
        })
    }))
}
