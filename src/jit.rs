use cranelift::{codegen::Context, prelude::FunctionBuilderContext};
use cranelift_jit::JITModule;
use cranelift_module::DataDescription;

pub struct JIT {
    // reuseable scratchpad
    builder_context: FunctionBuilderContext,
    // actual machine code
    ctx: Context,
    data_description: DataDescription,
    module: JITModule
}
