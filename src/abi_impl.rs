use crate::env::{get_remaining_points_for_env, sub_remaining_point, Env};
use crate::types::{Address, Response};
use crate::{settings, Bytecode};
use anyhow::Result;
use as_ffi_bindings::{Read as ASRead, StringPtr, Write as ASWrite};

#[derive(Debug, Clone)]
pub(crate) struct ExitCode(pub(crate) String);
impl std::fmt::Display for ExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ExitCode {}
macro_rules! abi_bail {
    ($err:expr) => {
        wasmer::RuntimeError::raise(Box::new(crate::abi_impl::ExitCode($err.to_string())))
    };
}
macro_rules! get_memory {
    ($env:ident) => {
        match $env.wasm_env.memory.get_ref() {
            Some(mem) => mem,
            _ => abi_bail!("uninitialized memory"),
        }
    };
}
pub(crate) use abi_bail;
pub(crate) use get_memory;

/// `Call` ABI called by the webassembly VM
///
/// Call an exported function in a WASM module at a given address
///
/// It take in argument the environment defined in env.rs
/// this environment is automatically filled by the wasmer library
/// And two pointers of string. (look at the readme in the wasm folder)
fn call_module(env: &Env, address: &Address, function: &str, param: &str) -> Result<Response> {
    let module = &env.interface.get_module(address)?;
    crate::execution_impl::exec(
        get_remaining_points_for_env(env),
        None,
        module,
        function,
        param,
        &*env.interface,
    )
}

fn create_sc(env: &Env, bytecode: &Bytecode) -> Result<Address> {
    env.interface.create_module(bytecode)
}

/// Raw call that have the right type signature to be able to be call a module
/// directly form AssemblyScript:
///
#[doc = include_str!("../wasm/README.md")]
pub(crate) fn assembly_script_call_module(
    env: &Env,
    address: i32,
    function: i32,
    param: i32,
) -> i32 {
    sub_remaining_point(env, settings::metering_call());
    let memory = get_memory!(env);
    let addr_ptr = StringPtr::new(address as u32);
    let func_ptr = StringPtr::new(function as u32);
    let param_ptr = StringPtr::new(param as u32);

    let address = addr_ptr.read(memory);
    let function = func_ptr.read(memory);
    let param = param_ptr.read(memory);
    if address.is_err() || function.is_err() || param.is_err() {
        abi_bail!("Cannot read address, function or param in memory in call module request ABI")
    }
    let address = &address.unwrap();
    let function = &function.unwrap();
    let param = &param.unwrap();
    let value = call_module(env, address, function, param);
    if value.is_err() {
        abi_bail!(value.err().unwrap())
    }
    if let Ok(ret) = StringPtr::alloc(&value.unwrap().ret, &env.wasm_env) {
        ret.offset() as i32
    } else {
        abi_bail!(format!(
            "Cannot allocate response in call {}::{}",
            address, function
        ))
    }
}

pub(crate) fn get_remaining_points(env: &Env) -> i32 {
    sub_remaining_point(env, settings::metering_remaining_points());
    get_remaining_points_for_env(env) as i32
}

/// Create an instance of VM from a module with a
/// given intefrace, an operation number limit and a webassembly module
///
/// An utility print function to write on stdout directly from AssemblyScript:
pub(crate) fn assembly_script_print(env: &Env, arg: i32) {
    sub_remaining_point(env, settings::metering_print());
    let str_ptr = StringPtr::new(arg as u32);
    let memory = get_memory!(env);
    if let Ok(message) = &str_ptr.read(memory) {
        if env.interface.print(message).is_err() {
            abi_bail!("Failed to print message");
        }
    } else {
        abi_bail!("Cannot read message pointer in memory");
    }
}

pub(crate) fn assembly_script_create_sc(env: &Env, bytecode: i32) -> i32 {
    sub_remaining_point(env, settings::metering_create_sc());
    let bytecode_ptr = StringPtr::new(bytecode as u32);
    let memory = get_memory!(env);
    let address = if let Ok(bytecode) = &bytecode_ptr.read(memory) {
        // Base64 to Binary
        let bytecode = base64::decode(bytecode);
        if bytecode.is_err() {
            abi_bail!("Failed to decode module");
        }
        if let Ok(address) = create_sc(env, &bytecode.unwrap()) {
            address
        } else {
            abi_bail!("Failed to create module smart contract");
        }
    } else {
        abi_bail!("Cannot read bytecode pointer in memory");
    };
    if let Ok(address_ptr) = StringPtr::alloc(&address, &env.wasm_env) {
        address_ptr.offset() as i32
    } else {
        abi_bail!("Cannot allocate address in memory")
    }
}

pub(crate) fn assembly_script_set_data(env: &Env, key: i32, value: i32) {
    sub_remaining_point(env, settings::metering_set_data());
    let memory = env.wasm_env.memory.get_ref().expect("uninitialized memory");
    let key = StringPtr::new(key as u32).read(memory);
    let value = StringPtr::new(value as u32).read(memory);
    if key.is_err() || value.is_err() {
        abi_bail!("Invalid pointer of key or value");
    }
    if let Err(err) = env
        .interface
        .set_data(&key.unwrap(), &value.unwrap().as_bytes().to_vec())
    {
        abi_bail!(err)
    }
}

pub(crate) fn assembly_script_get_data(env: &Env, key: i32) -> i32 {
    sub_remaining_point(env, settings::metering_get_data());
    let memory = env.wasm_env.memory.get_ref().expect("uninitialized memory");
    let key = StringPtr::new(key as u32).read(memory);
    if key.is_err() {
        abi_bail!("Invalid pointer of key");
    }
    let data = env.interface.get_data(&key.unwrap());
    if data.is_err() {
        abi_bail!("Failed to get data from ledger");
    }
    pointer_from_utf8(env, &data.unwrap()).offset() as i32
}

pub(crate) fn assembly_script_set_data_for(env: &Env, address: i32, key: i32, value: i32) {
    sub_remaining_point(env, settings::metering_set_data());
    let memory = env.wasm_env.memory.get_ref().expect("uninitialized memory");
    let address = StringPtr::new(address as u32).read(memory);
    let key = StringPtr::new(key as u32).read(memory);
    let value = StringPtr::new(value as u32).read(memory);
    if key.is_err() || value.is_err() || address.is_err() {
        abi_bail!("Invalid pointer of key, value or address");
    }
    if let Err(err) = env.interface.set_data_for(
        &address.unwrap(),
        &key.unwrap(),
        &value.unwrap().as_bytes().to_vec(),
    ) {
        abi_bail!(err)
    }
}

pub(crate) fn assembly_script_get_data_for(env: &Env, address: i32, key: i32) -> i32 {
    sub_remaining_point(env, settings::metering_get_data());
    let memory = env.wasm_env.memory.get_ref().expect("uninitialized memory");
    let address = StringPtr::new(address as u32).read(memory);
    let key = StringPtr::new(key as u32).read(memory);
    if key.is_err() || address.is_err() {
        abi_bail!("Invalid pointer of key or address");
    }
    let data = env.interface.get_data_for(&address.unwrap(), &key.unwrap());
    if data.is_err() {
        abi_bail!("Failed to get data from ledger");
    }
    pointer_from_utf8(env, &data.unwrap()).offset() as i32
}

/// Tooling, return a StringPtr allocated from a bytecode with utf8 parsing
///
fn pointer_from_utf8(env: &Env, bytecode: &Bytecode) -> StringPtr {
    match std::str::from_utf8(bytecode) {
        Ok(data) => match StringPtr::alloc(&data.to_string(), &env.wasm_env) {
            Ok(ptr) => *ptr,
            Err(err) => abi_bail!(err),
        },
        Err(err) => abi_bail!(err),
    }
}
