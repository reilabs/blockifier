use std::any::Any;
use std::collections::HashMap;

use cairo_felt::{Felt, FeltOps};
use cairo_vm::serde::deserialize_program::{
    deserialize_array_of_bigint_hex, deserialize_felt_hex, Attribute, HintParams, Identifier,
    ReferenceManager,
};
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::program::Program;
use cairo_vm::types::relocatable::{MaybeRelocatable, Relocatable};
use cairo_vm::vm::errors::memory_errors::MemoryError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::cairo_runner::CairoRunner;
use cairo_vm::vm::vm_core::VirtualMachine;
use starknet_api::core::{ClassHash, ContractAddress};
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::Calldata;

use crate::block_context::BlockContext;
use crate::execution::entry_point::{
    execute_constructor_entry_point, CallEntryPoint, CallExecution, CallInfo,
    EntryPointExecutionResult, Retdata,
};
use crate::execution::errors::{
    PostExecutionError, PreExecutionError, VirtualMachineExecutionError,
};
use crate::execution::syscall_handling::SyscallHintProcessor;
use crate::state::state_api::State;
use crate::transaction::objects::AccountTransactionContext;

pub type Args = Vec<Box<dyn Any>>;

#[cfg(test)]
#[path = "execution_utils_test.rs"]
pub mod test;

pub fn stark_felt_to_felt(stark_felt: StarkFelt) -> Felt {
    Felt::from_bytes_be(stark_felt.bytes())
}

pub fn felt_to_stark_felt(felt: &Felt) -> StarkFelt {
    let biguint = format!("{:#x}", felt.to_biguint());
    StarkFelt::try_from(biguint.as_str()).expect("Felt must be in StarkFelt's range.")
}

pub struct ExecutionContext<'a> {
    pub runner: CairoRunner,
    pub vm: VirtualMachine,
    pub syscall_handler: SyscallHintProcessor<'a>,
    pub initial_syscall_ptr: Relocatable,
    pub entry_point_pc: usize,
}

pub fn initialize_execution_context<'a>(
    call_entry_point: &CallEntryPoint,
    class_hash: ClassHash,
    state: &'a mut dyn State,
    block_context: &'a BlockContext,
    account_tx_context: &'a AccountTransactionContext,
) -> Result<ExecutionContext<'a>, PreExecutionError> {
    let contract_class = state.get_contract_class(&class_hash)?;

    // Resolve initial PC from EP indicator.
    let entry_point_pc = call_entry_point.resolve_entry_point_pc(contract_class)?;

    // Instantiate Cairo runner.
    let program = convert_program_to_cairo_runner_format(&contract_class.program)?;
    let mut runner = CairoRunner::new(&program, "all", false)?;
    let mut vm = VirtualMachine::new(false);
    runner.initialize_builtins(&mut vm)?;
    runner.initialize_segments(&mut vm, None);

    // Instantiate syscall handler.
    let initial_syscall_ptr = vm.add_memory_segment();
    let syscall_handler = SyscallHintProcessor::new(
        state,
        block_context,
        account_tx_context,
        initial_syscall_ptr,
        call_entry_point.storage_address,
        call_entry_point.caller_address,
    );

    Ok(ExecutionContext { runner, vm, syscall_handler, initial_syscall_ptr, entry_point_pc })
}

pub fn prepare_call_arguments(
    call_entry_point: &CallEntryPoint,
    vm: &mut VirtualMachine,
    initial_syscall_ptr: Relocatable,
    read_only_segments: &mut ReadOnlySegments,
) -> Result<(Vec<MaybeRelocatable>, Args), PreExecutionError> {
    let mut args: Args = vec![];

    // Prepare called EP details.
    let entry_point_selector =
        MaybeRelocatable::Int(stark_felt_to_felt(call_entry_point.entry_point_selector.0));
    args.push(Box::new(entry_point_selector));

    // Prepare implicit arguments.
    let mut implicit_args = vec![];
    implicit_args.push(initial_syscall_ptr.into());
    implicit_args.extend(
        vm.get_builtin_runners()
            .iter()
            .flat_map(|(_name, builtin_runner)| builtin_runner.initial_stack()),
    );
    args.push(Box::new(implicit_args.clone()));

    // Prepare calldata arguments.
    let calldata = &call_entry_point.calldata.0;
    let calldata: Vec<MaybeRelocatable> =
        calldata.iter().map(|arg| MaybeRelocatable::Int(stark_felt_to_felt(*arg))).collect();
    args.push(Box::new(MaybeRelocatable::Int(Felt::from(calldata.len()))));
    let calldata_start_ptr =
        MaybeRelocatable::RelocatableValue(read_only_segments.allocate(vm, calldata)?);
    args.push(Box::new(calldata_start_ptr));

    Ok((implicit_args, args))
}

/// Executes a specific call to a contract entry point and returns its output.
pub fn execute_entry_point_call(
    call_entry_point: CallEntryPoint,
    class_hash: ClassHash,
    state: &mut dyn State,
    block_context: &BlockContext,
    account_tx_context: &AccountTransactionContext,
) -> EntryPointExecutionResult<CallInfo> {
    let mut execution_context = initialize_execution_context(
        &call_entry_point,
        class_hash,
        state,
        block_context,
        account_tx_context,
    )?;
    let (implicit_args, args) = prepare_call_arguments(
        &call_entry_point,
        &mut execution_context.vm,
        execution_context.initial_syscall_ptr,
        &mut execution_context.syscall_handler.read_only_segments,
    )?;

    run_entry_point(
        &mut execution_context.runner,
        &mut execution_context.vm,
        execution_context.entry_point_pc,
        args,
        &mut execution_context.syscall_handler,
    )?;

    Ok(finalize_execution(
        execution_context.vm,
        call_entry_point,
        execution_context.syscall_handler,
        implicit_args,
    )?)
}

pub fn run_entry_point(
    cairo_runner: &mut CairoRunner,
    vm: &mut VirtualMachine,
    entry_point_pc: usize,
    args: Args,
    hint_processor: &mut SyscallHintProcessor<'_>,
) -> Result<(), VirtualMachineExecutionError> {
    cairo_runner.run_from_entrypoint(
        entry_point_pc,
        args.iter().map(|x| x.as_ref()).collect(),
        false,
        true,
        true,
        vm,
        hint_processor,
    )?;
    Ok(())
}

pub fn finalize_execution(
    mut vm: VirtualMachine,
    call_entry_point: CallEntryPoint,
    syscall_handler: SyscallHintProcessor<'_>,
    implicit_args: Vec<MaybeRelocatable>,
) -> Result<CallInfo, PostExecutionError> {
    let [retdata_size, retdata_ptr]: [MaybeRelocatable; 2] =
        vm.get_return_values(2)?.try_into().expect("Return values must be of size 2.");
    let implicit_args_end_ptr = vm.get_ap().sub_usize(2)?;
    validate_run(&mut vm, implicit_args, implicit_args_end_ptr, &syscall_handler)?;
    syscall_handler.read_only_segments.mark_as_accessed(&mut vm)?;

    Ok(CallInfo {
        call: call_entry_point,
        execution: CallExecution {
            retdata: read_execution_retdata(vm, retdata_size, retdata_ptr)?,
        },
        inner_calls: syscall_handler.inner_calls,
        events: syscall_handler.events,
        l2_to_l1_messages: syscall_handler.l2_to_l1_messages,
    })
}

pub fn validate_run(
    vm: &mut VirtualMachine,
    implicit_args: Vec<MaybeRelocatable>,
    implicit_args_end: Relocatable,
    syscall_handler: &SyscallHintProcessor<'_>,
) -> Result<(), PostExecutionError> {
    // Validate builtins' final stack.
    let mut current_builtin_ptr = implicit_args_end;
    for (_name, builtin_runner) in vm.get_builtin_runners().iter().rev() {
        // Validates builtin segment and returns a pointer to the previous segment.
        (current_builtin_ptr, _) = builtin_runner.final_stack(vm, current_builtin_ptr)?;
    }

    // Validate implicit arguments segment length is unchanged.
    // Subtract one to get to the first implicit arg segment (the syscall pointer).
    let implicit_args_start = current_builtin_ptr.sub_usize(1)?;
    if implicit_args_start + implicit_args.len() != implicit_args_end {
        return Err(PostExecutionError::SecurityValidationError(
            "Implicit arguments' segments".to_string(),
        ));
    }

    // Validate syscall segment start.
    let syscall_start_ptr = implicit_args.first().expect("Implicit args must not be empty.");
    let syscall_start_ptr = Relocatable::try_from(syscall_start_ptr)?;
    if syscall_start_ptr.offset != 0 {
        return Err(PostExecutionError::SecurityValidationError(
            "Syscall segment start".to_string(),
        ));
    }

    // Validate syscall segment size.
    let syscall_end_ptr = vm.get_relocatable(&implicit_args_start)?;
    let syscall_used_size = vm
        .get_segment_used_size(syscall_start_ptr.segment_index as usize)
        .expect("Segments must contain the syscall segment.");
    if syscall_start_ptr + syscall_used_size != syscall_end_ptr {
        return Err(PostExecutionError::SecurityValidationError(
            "Syscall segment size".to_string(),
        ));
    }

    // Validate syscall segment end.
    syscall_handler.verify_syscall_ptr(syscall_end_ptr).map_err(|_| {
        PostExecutionError::SecurityValidationError("Syscall segment end".to_string())
    })?;

    syscall_handler.read_only_segments.validate(vm)
}

fn read_execution_retdata(
    vm: VirtualMachine,
    retdata_size: MaybeRelocatable,
    retdata_ptr: MaybeRelocatable,
) -> Result<Retdata, PostExecutionError> {
    let retdata_size = match retdata_size {
        // TODO(AlonH, 21/12/2022): Handle case where res_data_size is larger than usize.
        MaybeRelocatable::Int(retdata_size) => retdata_size.bits() as usize,
        relocatable => return Err(VirtualMachineError::ExpectedInteger(relocatable).into()),
    };

    Ok(Retdata(felt_range(&vm, &retdata_ptr, retdata_size)?.into()))
}

pub fn felt_range(
    vm: &VirtualMachine,
    ptr: &MaybeRelocatable,
    size: usize,
) -> Result<Vec<StarkFelt>, VirtualMachineError> {
    let values = vm.get_continuous_range(ptr, size)?;
    // Extract values as `StarkFelt`.
    let values: Result<Vec<StarkFelt>, VirtualMachineError> =
        values.into_iter().map(|x| get_felt_from_memory_cell(Some(x))).collect();
    values
}

// TODO(Noa, 01/12/2022): Change this temporary solution.
pub fn convert_program_to_cairo_runner_format(
    program: &starknet_api::state::Program,
) -> Result<Program, ProgramError> {
    let program = program.clone();
    let identifiers = serde_json::from_value::<HashMap<String, Identifier>>(program.identifiers)?;

    let start = match identifiers.get("__main__.__start__") {
        Some(identifier) => identifier.pc,
        None => None,
    };
    let end = match identifiers.get("__main__.__end__") {
        Some(identifier) => identifier.pc,
        None => None,
    };

    Ok(Program {
        builtins: serde_json::from_value::<Vec<String>>(program.builtins)?,
        prime: deserialize_felt_hex(program.prime)?.to_string(),
        data: deserialize_array_of_bigint_hex(program.data)?,
        constants: {
            let mut constants = HashMap::new();
            for (key, value) in identifiers.iter() {
                if value.type_.as_deref() == Some("const") {
                    let value = value
                        .value
                        .clone()
                        .ok_or_else(|| ProgramError::ConstWithoutValue(key.to_owned()))?;
                    constants.insert(key.to_owned(), value);
                }
            }

            constants
        },
        main: None,
        start,
        end,
        hints: serde_json::from_value::<HashMap<usize, Vec<HintParams>>>(program.hints)?,
        reference_manager: serde_json::from_value::<ReferenceManager>(program.reference_manager)?,
        identifiers,
        error_message_attributes: serde_json::from_value::<Vec<Attribute>>(program.attributes)?
            .into_iter()
            .filter(|attr| attr.name == "error_message")
            .collect(),
        instruction_locations: None,
    })
}

pub fn get_felt_from_memory_cell(
    memory_cell: Option<MaybeRelocatable>,
) -> Result<StarkFelt, VirtualMachineError> {
    match memory_cell {
        Some(MaybeRelocatable::Int(value)) => Ok(felt_to_stark_felt(&value)),
        Some(relocatable) => Err(VirtualMachineError::ExpectedInteger(relocatable)),
        None => Err(VirtualMachineError::NoneInMemoryRange),
    }
}

/// Represents read-only segments dynamically allocated during execution.
#[derive(Debug, Default)]
pub struct ReadOnlySegments(Vec<(Relocatable, usize)>);

impl ReadOnlySegments {
    pub fn allocate(
        &mut self,
        vm: &mut VirtualMachine,
        data: Vec<MaybeRelocatable>,
    ) -> Result<Relocatable, MemoryError> {
        let segment_start_ptr = vm.add_memory_segment();
        self.0.push((segment_start_ptr, data.len()));
        vm.load_data(&segment_start_ptr.into(), &data)?;
        Ok(segment_start_ptr)
    }

    pub fn validate(&self, vm: &mut VirtualMachine) -> Result<(), PostExecutionError> {
        // TODO(AlonH, 21/12/2022): Validate segments consistency ("assert self.segments is
        // runner.segments" in python).
        for (segment_start_ptr, segment_size) in &self.0 {
            let used_size = vm
                .get_segment_used_size(segment_start_ptr.segment_index as usize)
                .expect("Segments must contain the allocated read-only segment.");
            if *segment_size != used_size {
                return Err(PostExecutionError::SecurityValidationError(
                    "Read-only segments".to_string(),
                ));
            }
        }

        Ok(())
    }

    pub fn mark_as_accessed(self, vm: &mut VirtualMachine) -> Result<(), PostExecutionError> {
        for (segment_start_ptr, segment_size) in self.0 {
            vm.mark_address_range_as_accessed(segment_start_ptr, segment_size)?;
        }

        Ok(())
    }
}

/// Instantiates the given class and assigns it an address.
/// Returns the call info of the deployed class' constructor execution.
pub fn execute_deployment(
    state: &mut dyn State,
    block_context: &BlockContext,
    account_tx_context: &AccountTransactionContext,
    class_hash: ClassHash,
    deployed_contract_address: ContractAddress,
    deployer_address: ContractAddress,
    constructor_calldata: Calldata,
) -> EntryPointExecutionResult<CallInfo> {
    // Address allocation in the state is done before calling the constructor, so that it is
    // visible from it.
    state.set_class_hash_at(deployed_contract_address, class_hash)?;
    execute_constructor_entry_point(
        state,
        block_context,
        account_tx_context,
        class_hash,
        deployed_contract_address,
        deployer_address,
        constructor_calldata,
    )
}