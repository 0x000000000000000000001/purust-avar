pub fn Test_Probe_watchdog() -> UnknownType {
    Value::Func1(purust_core::Func1::Static(|_| {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(15));
            eprintln!("AVar tests timed out after 15 seconds");
            std::process::exit(124);
        });
        Value::Unit
    }))
}
