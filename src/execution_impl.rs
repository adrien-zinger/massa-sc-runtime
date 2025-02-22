use crate::settings;
use crate::types::{Interface, Response};
use crate::{abi_impl::*, tunable_memory::LimitingTunables};
use crate::{
    env::{assembly_script_abort, get_remaining_points, Env},
    settings::max_number_of_pages,
};
use anyhow::{bail, Result};
use as_ffi_bindings::{Read as ASRead, StringPtr, Write as ASWrite};
use std::sync::Arc;
use wasmer::WasmerEnv;
use wasmer::{
    imports, CompilerConfig, Features, Function, ImportObject, Instance, Module, Store, Universal,
    Val,
};
use wasmer::{wasmparser::Operator, BaseTunables, Pages, Target};
use wasmer_compiler_singlepass::Singlepass;
use wasmer_middlewares::metering::{self, MeteringPoints};
use wasmer_middlewares::Metering;

/// Create an instance of VM from a module with a given interface, an operation
/// number limit and a webassembly module
fn create_instance(limit: u64, module: &[u8], env: &Env) -> Result<Instance> {
    // We use the Singlepass compiler because it is fast and adapted to blockchains
    // See https://docs.rs/wasmer-compiler-singlepass/latest/wasmer_compiler_singlepass/
    let mut compiler_config = Singlepass::new();

    // Turning-off sources of potential non-determinism,
    // see https://github.com/WebAssembly/design/blob/037c6fe94151eb13e30d174f5f7ce851be0a573e/Nondeterminism.md

    // Turning-off in the compiler:

    // Canonicalize NaN.
    compiler_config.canonicalize_nans(true);

    // enable stack check
    compiler_config.enable_stack_check(true);

    // Turning-off in wasmer feature flags:
    let mut features = Features::new();

    // Disable threads.
    features.threads(false);

    // Turn-off experimental SIMD feature.
    features.simd(false);

    // Turn-off multivalue, because it is not supported for Singlepass(and it's true by default).
    features.multi_value(false);

    // Add metering middleware
    let metering = Arc::new(Metering::new(limit, |_: &Operator| -> u64 { 1 }));
    compiler_config.push_middleware(metering);

    let base = BaseTunables::for_target(&Target::default());
    let tunables = LimitingTunables::new(base, Pages(max_number_of_pages()));
    let engine = Universal::new(compiler_config).features(features).engine();
    let store = Store::new_with_tunables(&engine, tunables);
    let resolver: ImportObject = imports! {
        "env" => {
            // Needed by wasm generated by AssemblyScript.
            "abort" =>  Function::new_native_with_env(&store, env.clone(), assembly_script_abort),
        },
        "massa" => {
            "assembly_script_print" => Function::new_native_with_env(&store, env.clone(), assembly_script_print),
            "assembly_script_call" => Function::new_native_with_env(&store, env.clone(), assembly_script_call_module),
            "assembly_script_get_remaining_gas" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_remaining_gas),
            "assembly_script_create_sc" => Function::new_native_with_env(&store, env.clone(), assembly_script_create_sc),
            "assembly_script_set_data" => Function::new_native_with_env(&store, env.clone(), assembly_script_set_data),
            "assembly_script_set_data_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_set_data_for),
            "assembly_script_get_data" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_data),
            "assembly_script_get_data_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_data_for),
            "assembly_script_delete_data" => Function::new_native_with_env(&store, env.clone(), assembly_script_delete_data),
            "assembly_script_delete_data_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_delete_data_for),
            "assembly_script_append_data" => Function::new_native_with_env(&store, env.clone(), assembly_script_append_data),
            "assembly_script_append_data_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_append_data_for),
            "assembly_script_has_data" => Function::new_native_with_env(&store, env.clone(), assembly_script_has_data),
            "assembly_script_has_data_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_has_data_for),
            "assembly_script_get_owned_addresses" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_owned_addresses),
            "assembly_script_get_owned_addresses_raw" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_owned_addresses_raw),
            "assembly_script_get_call_stack" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_call_stack),
            "assembly_script_get_call_stack_raw" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_call_stack_raw),
            "assembly_script_generate_event" => Function::new_native_with_env(&store, env.clone(), assembly_script_generate_event),
            "assembly_script_transfer_coins" => Function::new_native_with_env(&store, env.clone(), assembly_script_transfer_coins),
            "assembly_script_transfer_coins_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_transfer_coins_for),
            "assembly_script_get_balance" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_balance),
            "assembly_script_get_balance_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_balance_for),
            "assembly_script_hash" => Function::new_native_with_env(&store, env.clone(), assembly_script_hash),
            "assembly_script_signature_verify" => Function::new_native_with_env(&store, env.clone(), assembly_script_signature_verify),
            "assembly_script_address_from_public_key" => Function::new_native_with_env(&store, env.clone(), assembly_script_address_from_public_key),
            "assembly_script_unsafe_random" => Function::new_native_with_env(&store, env.clone(), assembly_script_unsafe_random),
            "assembly_script_get_call_coins" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_call_coins),
            "assembly_script_get_time" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_time),
            "assembly_script_send_message" => Function::new_native_with_env(&store, env.clone(), assembly_script_send_message),
            "assembly_script_get_current_period" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_current_period),
            "assembly_script_get_current_thread" => Function::new_native_with_env(&store, env.clone(), assembly_script_get_current_thread),
            "assembly_script_set_bytecode" => Function::new_native_with_env(&store, env.clone(), assembly_script_set_bytecode),
            "assembly_script_set_bytecode_for" => Function::new_native_with_env(&store, env.clone(), assembly_script_set_bytecode_for),
        },
    };
    let module = Module::new(&store, &module)?;
    Ok(Instance::new(&module, &resolver)?)
}

pub(crate) fn exec(
    limit: u64,
    instance: Option<Instance>,
    module: &[u8],
    function: &str,
    param: &str,
    interface: &dyn Interface,
) -> Result<Response> {
    let mut env = Env::new(interface);
    let instance = match instance {
        Some(instance) => instance,
        None => create_instance(limit, module, &env)?,
    };
    env.init_with_instance(&instance)?;

    // Closure for the execution allowing us to handle a gas error
    fn execution(instance: &Instance, function: &str, param: &str, env: &Env) -> Result<Response> {
        let param_ptr = *StringPtr::alloc(&param.to_string(), &env.wasm_env)?;
        match instance
            .exports
            .get_function(function)?
            .call(&[Val::I32(param_ptr.offset() as i32)])
        {
            Ok(value) => {
                // TODO: clean and define wat should be return by the main
                if function.eq(crate::settings::MAIN) {
                    return Ok(Response {
                        ret: "0".to_string(),
                        remaining_gas: get_remaining_points(env)?,
                    });
                }
                let ret = if let Some(offset) = value.get(0) {
                    if let Some(offset) = offset.i32() {
                        let str_ptr = StringPtr::new(offset as u32);
                        let memory = instance.exports.get_memory("memory")?;
                        str_ptr.read(memory)?
                    } else {
                        bail!("Execution wasn't in capacity to read the return value")
                    }
                } else {
                    String::new()
                };
                Ok(Response {
                    ret,
                    remaining_gas: get_remaining_points(env)?,
                })
            }
            Err(error) => bail!(error),
        }
    }

    match execution(&instance, function, param, &env) {
        Ok(response) => Ok(response),
        Err(err) => {
            // Because the last needed more than the remaining points, we should have an error.
            match metering::get_remaining_points(&instance) {
                MeteringPoints::Remaining(..) => bail!(err),
                MeteringPoints::Exhausted => bail!("Not enough gas, limit reached at: {function}"),
            }
        }
    }
}

/// Library Input, take a `module` wasm builded with the massa environment,
/// must have a main function inside written in AssemblyScript:
///
/// ```js
/// import { print } from "massa-sc-std";
///
/// export function main(_args: string): i32 {
///     print("hello world");
///     return 0;
/// }
/// ```  
pub fn run_main(module: &[u8], limit: u64, interface: &dyn Interface) -> Result<u64> {
    let env = Env::new(interface);
    let instance = create_instance(limit, module, &env)?;
    if instance.exports.contains(settings::MAIN) {
        Ok(exec(limit, Some(instance), module, settings::MAIN, "", interface)?.remaining_gas)
    } else {
        Ok(limit)
    }
}

/// Library Input, take a `module` wasm builded with the massa environment,
/// run a function of that module with the given parameter:
///
/// ```js
/// import { print } from "massa-sc-std";
///
/// export function hello_world(_args: string): i32 {
///     print("hello world");
///     return 0;
/// }
/// ```  
pub fn run_function(
    module: &[u8],
    limit: u64,
    function: &str,
    param: &str,
    interface: &dyn Interface,
) -> Result<u64> {
    Ok(exec(limit, None, module, function, param, interface)?.remaining_gas)
}
