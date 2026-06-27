use cranelift::{codegen::Context, prelude::FunctionBuilderContext};

pub struct JIT {
    // reuseable scratchpad
    builder_context: FunctionBuilderContext,
    // actual machine code
    ctx: Context,
    data_description: DataDesc
}