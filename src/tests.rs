/// THIS FILE SHOULD TEST THE ABI, NOT THE MOCKED INTERFACE
use crate::{
    run, settings,
    types::{Interface, InterfaceClone},
};
use anyhow::{bail, Result};
use serial_test::serial;
use std::sync::{Arc, Mutex};
pub type Ledger = std::collections::BTreeMap<String, Vec<u8>>; // Byttecode instead of String

#[derive(Clone)]
struct TestInterface(Arc<Mutex<Ledger>>);

impl InterfaceClone for TestInterface {
    fn clone_box(&self) -> Box<dyn Interface> {
        Box::new(self.clone())
    }
}

impl Interface for TestInterface {
    fn init_call(&self, address: &str, _raw_coins: u64) -> Result<Vec<u8>> {
        match self
            .0
            .lock()
            .unwrap()
            .clone()
            .get::<String>(&address.to_string())
        {
            Some(module) => Ok(module.clone()),
            _ => bail!("Cannot find module for address {}", address),
        }
    }

    fn finish_call(&self) -> Result<()> {
        Ok(())
    }

    fn get_balance(&self) -> Result<u64> {
        Ok(1)
    }

    fn get_balance_for(&self, _address: &str) -> Result<u64> {
        Ok(1)
    }

    fn update_module(&self, address: &str, module: &[u8]) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .insert(address.to_string(), module.to_vec());
        Ok(())
    }

    fn print(&self, message: &str) -> Result<()> {
        println!("{}", message);
        self.0
            .lock()
            .unwrap()
            .insert("print".into(), message.as_bytes().to_vec());
        Ok(())
    }

    fn raw_get_data(&self, _: &str) -> Result<Vec<u8>> {
        match self.0.lock().unwrap().clone().get(&"print".to_string()) {
            Some(bytes) => Ok(bytes.clone()),
            _ => bail!("Cannot find data"),
        }
    }

    fn get_call_coins(&self) -> Result<u64> {
        Ok(0)
    }

    fn create_module(&self, module: &[u8]) -> Result<String> {
        let address = String::from("get_string");
        self.0
            .lock()
            .unwrap()
            .insert(address.clone(), module.to_vec());
        Ok(address)
    }
}

#[test]
#[serial]
fn test_caller() {
    settings::reset_metering();
    let interface: Box<dyn Interface> =
        Box::new(TestInterface(Arc::new(Mutex::new(Ledger::new()))));
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/get_string.wat"
    ));
    interface
        .update_module("get_string", module.as_ref())
        .unwrap();
    // test only if the module is valid
    run(module, 20_000, &*interface).expect("Failed to run get_string.wat");
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/caller.wat"
    ));
    let a = run(module, 20_000, &*interface).expect("Failed to run caller.wat");
    let prev_call_price = settings::metering_call();
    settings::set_metering(0);
    let b = run(module, 20_000, &*interface).expect("Failed to run caller.wat");
    assert_eq!(a + prev_call_price, b);
    let v_out = interface.raw_get_data("").unwrap();
    let output = std::str::from_utf8(&v_out).unwrap();
    assert_eq!(output, "hello you");

    // Test now if we failed if metering is too hight
    settings::set_metering(15_000);
    run(module, 20_000, &*interface).expect_err("Expected to be out of operation gas");
}

#[test]
#[serial]
fn test_caller_no_return() {
    settings::reset_metering();
    let interface: Box<dyn Interface> =
        Box::new(TestInterface(Arc::new(Mutex::new(Ledger::new()))));
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/get_string.wat"
    ));
    interface
        .update_module("get_string", module.as_ref())
        .unwrap();
    // test only if the module is valid
    run(module, 20_000, &*interface).expect("Failed to run get_string.wat");
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/caller_no_return.wasm"
    ));
    run(module, 20_000, &*interface).expect("Failed to run caller.wat");
}

#[test]
#[serial]
fn test_local_hello_name_caller() {
    settings::reset_metering();
    // This test should verify that even if we failed to load a module,
    // we should never panic and just stop the call stack
    let interface: Box<dyn Interface> =
        Box::new(TestInterface(Arc::new(Mutex::new(Ledger::new()))));
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/get_string.wat"
    ));
    interface
        .update_module("get_string", module.as_ref())
        .unwrap();
    run(module, 100, &*interface).expect("Failed to run get_string.wat");
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/local_hello_name_caller.wat"
    ));
    run(module, 20_000, &*interface).expect_err("Succeeded to run local_hello_name_caller.wat");
}

#[test]
#[serial]
fn test_module_creation() {
    settings::reset_metering();
    // This test should create a smartcontract module and call it
    let interface: Box<dyn Interface> =
        Box::new(TestInterface(Arc::new(Mutex::new(Ledger::new()))));
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/create_sc.wasm"
    ));
    run(module, 100_000, &*interface).expect("Failed to run create_sc.wat");
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/caller.wat"
    ));
    run(module, 20_000, &*interface).expect("Failed to run caller.wat");
}

#[test]
#[serial]
fn test_not_enough_gas_error() {
    settings::reset_metering();
    // This test should create a smartcontract module and call it
    let interface: Box<dyn Interface> =
        Box::new(TestInterface(Arc::new(Mutex::new(Ledger::new()))));
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/create_sc.wasm"
    ));
    run(module, 100_000, &*interface).expect("Failed to run create_sc.wat");
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/caller.wat"
    ));
    match run(module, 10000, &*interface) {
        Ok(_) => panic!("Shouldn't pass successfully =-("),
        Err(err) => {
            assert!(err
                .to_string()
                .starts_with("RuntimeError: Not enough gas, limit reached at:"))
        }
    }
}

#[test]
#[serial]
fn test_run_without_main() {
    settings::reset_metering();
    let interface: Box<dyn Interface> =
        Box::new(TestInterface(Arc::new(Mutex::new(Ledger::new()))));
    let module = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wasm/build/no_main.wasm"
    ));
    run(module, 100_000, &*interface).expect_err("An error should spawn here");
}
