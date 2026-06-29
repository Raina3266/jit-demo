use crate::parser::*;
use cranelift::codegen::Context;
use cranelift::codegen::ir::BlockArg;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};
use std::collections::HashMap;

pub struct JIT {
    // reuseable scratchpad
    builder_context: FunctionBuilderContext,
    // actual machine code
    ctx: Context,
    data_description: DataDescription,
    module: JITModule
}

// I guess we can just directly copy from the demo without understanding too much
// like JIT::new()
//
impl Default for JIT {
    fn default() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
        }
    }
}

impl JIT {
    /// Compile a string in the language into machine code.
    pub fn compile(&mut self, input: &str) -> Result<*const u8, String> {
        todo!()
    }

    // Translate from toy-language AST nodes into Cranelift IR.
    fn translate(&mut self, f: &Function) -> Result<(), String> {

        // Signature: (i64, i64, ...) -> i64
        let int = types::I64;
        self.ctx.func.signature.params = vec![AbiParam::new(int); f.params.len()];
        self.ctx.func.signature.returns = vec![AbiParam::new(int)];

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let variables =
            declare_variables(int, &mut builder, &f.params, &f.body.stmts, entry_block);
        todo!()

        
    }
}

fn declare_variables(
    int: types::Type,
    builder: &mut FunctionBuilder,
    params: &[String],
    stmts: &[Stmt],
    entry_block: Block,
) -> HashMap<String, Variable> {
    let mut variables = HashMap::new();

    // several input params
    for (i, name) in params.iter().enumerate() {
        let val = builder.block_params(entry_block)[i];
        let var = declare_variable(int, builder, &mut variables, name);
        // name + type
        builder.def_var(var, val);
    }

    // one output param (in our function we don't have return variable)

    // several param in body
    for stmt in stmts {
        declare_variables_in_stmt(int, builder, &mut variables, stmt);
    }
    variables
}

/// Declare a single variable declaration.
fn declare_variable(
    int: types::Type,
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<String, Variable>,
    name: &str,
) -> Variable {
   *variables.entry(name.into()).or_insert_with(|| builder.declare_var(int))
}

fn declare_variables_in_stmt(
    int: types::Type,
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<String, Variable>,
    stmt: &Stmt,
) {
    match stmt {
        Stmt::Let { name, value } => {
            declare_variable(int, builder, variables, name);
        }
        Stmt::If {cond, then, else_} => {
            for stmt in then.stmts.iter() {
                declare_variables_in_stmt(int, builder, variables, stmt);
            }
            if let Some(has_else) = else_ {
                declare_variables_in_stmt(int, builder, variables, has_else);
            }
        }
        _ => (),
    }
}
