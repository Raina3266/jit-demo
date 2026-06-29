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
    module: JITModule,
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

        let variables = declare_variables(int, &mut builder, &f.params, &f.body.stmts, entry_block);
        let mut trans = FunctionTranslator {
            int,
            builder,
            variables,
            module: &mut self.module,
        };
        for stmt in &f.body.stmts {
            trans.translate_stmt(stmt);
        }

        // Tell the builder we're done with this function.
        trans.builder.finalize();
        Ok(())
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
    *variables
        .entry(name.into())
        .or_insert_with(|| builder.declare_var(int))
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
        Stmt::If { cond, then, else_ } => {
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

// A collection of state used for translating from AST nodes into Cranelift IR.
struct FunctionTranslator<'a> {
    int: types::Type,
    builder: FunctionBuilder<'a>,
    variables: HashMap<String, Variable>,
    module: &'a mut JITModule,
}

impl<'a> FunctionTranslator<'a> {
    /// When you write out instructions in Cranelift, you get back `Value`s. You
    /// can then use these references in other instructions.
    fn translate_expr(&mut self, expr: Expr) -> Value {
        match expr {
            Expr::Int(n) => self.builder.ins().iconst(self.int, n),
            Expr::Var(name) => {
                let var = self.variables.get(&name).expect("variable not defined");
                self.builder.use_var(*var)
            }

            Expr::Call { name, args } => {
                let arg_vals: Vec<Value> =
                    args.into_iter().map(|a| self.translate_expr(a)).collect();

                let mut sig = self.module.make_signature();
                sig.params = vec![AbiParam::new(self.int); arg_vals.len()];
                sig.returns = vec![AbiParam::new(self.int)];
                let callee = self
                    .module
                    .declare_function(&name, Linkage::Import, &sig)
                    .expect("failed to declare function");
                // `call` needs a `FuncRef` (function-local handle), not the
                // module-wide `FuncId`. Convert it.
                let callee_ref = self.module.declare_func_in_func(callee, self.builder.func);

                // `call` returns an `Inst` handle, not a `Value`, because a call
                // may return zero, one, or many values. Extract the single i64
                // return value our functions always produce.
                let call = self.builder.ins().call(callee_ref, &arg_vals);
                self.builder.inst_results(call)[0]
            }

            Expr::Unary { op, expr } => {
                let val = self.translate_expr(*expr);
                match op {
                    UnaryOp::Neg => self.builder.ins().ineg(val),
                    UnaryOp::Not => {
                        let zero = self.builder.ins().iconst(self.int, 0);
                        self.cmp_to_i64(IntCC::Equal, val, zero)
                    }
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs_val = self.translate_expr(*lhs);
                let rhs_val = self.translate_expr(*rhs);
                match op {
                    BinaryOp::Add => self.builder.ins().iadd(lhs_val, rhs_val),
                    BinaryOp::Sub => self.builder.ins().isub(lhs_val, rhs_val),
                    BinaryOp::Mul => self.builder.ins().imul(lhs_val, rhs_val),
                    BinaryOp::Div => self.builder.ins().sdiv(lhs_val, rhs_val),
                    BinaryOp::Eq => self.cmp_to_i64(IntCC::Equal, lhs_val, rhs_val),
                    BinaryOp::Ne => self.cmp_to_i64(IntCC::NotEqual, lhs_val, rhs_val),
                    BinaryOp::Lt => self.cmp_to_i64(IntCC::SignedLessThan, lhs_val, rhs_val),
                    BinaryOp::Le => self.cmp_to_i64(IntCC::SignedLessThanOrEqual, lhs_val, rhs_val),
                    BinaryOp::Gt => self.cmp_to_i64(IntCC::SignedGreaterThan, lhs_val, rhs_val),
                    BinaryOp::Ge => {
                        self.cmp_to_i64(IntCC::SignedGreaterThanOrEqual, lhs_val, rhs_val)
                    }
                }
            }
        }
    }

    fn translate_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value } => {
                let val = self.translate_expr(value.clone());
                let var = *self.variables.get(name).expect("variable not declared");
                self.builder.def_var(var, val);
            }
            Stmt::Return(expr) => {
                let val = self.translate_expr(expr.clone());
                self.builder.ins().return_(&[val]);
            }
            Stmt::Expr(expr) => {
                // Evaluate for side effects (e.g. function calls), discard result.
                self.translate_expr(expr.clone());
            }
            Stmt::If { cond, then, else_ } => {
                let cond_val = self.translate_expr(cond.clone());

                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .ins()
                    .brif(cond_val, then_block, &[], else_block, &[]);

                // then branch
                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                for s in &then.stmts {
                    self.translate_stmt(s);
                }
                self.builder.ins().jump(merge_block, &[]);

                // else branch
                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                match else_ {
                    Some(else_stmt) => self.translate_stmt(else_stmt),
                    None => {}
                }
                self.builder.ins().jump(merge_block, &[]);

                // merge
                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);
            }
        }
    }

    /// Helper: comparisons produce an i8 boolean; widen it to i64 so the rest
    /// of the language can treat the result like any other value. Booleans
    /// are 0 or 1, so zero-extension is correct.
    fn cmp_to_i64(&mut self, cc: IntCC, lhs: Value, rhs: Value) -> Value {
        let cmp = self.builder.ins().icmp(cc, lhs, rhs);
        self.builder.ins().uextend(self.int, cmp)
    }
}
