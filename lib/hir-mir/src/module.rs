use super::{context::CompileContext, expression, generic_type_definition, type_, CompileError};
use crate::spawn_function_declaration;
use hir::{analysis::AnalysisError, ir::*};

pub fn compile(context: &CompileContext, module: &Module) -> Result<mir::ir::Module, CompileError> {
    Ok(mir::ir::Module::new(
        module
            .type_definitions()
            .iter()
            .map(|type_definition| compile_type_definition(type_definition, context))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .chain(generic_type_definition::compile(module, context)?)
            .collect(),
        module
            .foreign_declarations()
            .iter()
            .map(|declaration| -> Result<_, CompileError> {
                Ok(mir::ir::ForeignDeclaration::new(
                    declaration.name(),
                    declaration.foreign_name(),
                    type_::compile(context, declaration.type_())?
                        .into_function()
                        .ok_or_else(|| {
                            AnalysisError::FunctionExpected(declaration.position().clone())
                        })?,
                    compile_calling_convention(declaration.calling_convention()),
                ))
            })
            .chain(context.configuration().ok().map(|configuration| {
                Ok(spawn_function_declaration::compile(
                    &configuration.concurrency,
                ))
            }))
            .collect::<Result<_, _>>()?,
        module
            .function_definitions()
            .iter()
            .flat_map(|definition| {
                definition
                    .foreign_definition_configuration()
                    .map(|configuration| {
                        mir::ir::ForeignDefinition::new(
                            definition.name(),
                            definition.original_name(),
                            compile_calling_convention(configuration.calling_convention()),
                        )
                    })
            })
            .collect(),
        module
            .function_declarations()
            .iter()
            .map(|declaration| compile_function_declaration(declaration, context))
            .collect::<Result<_, _>>()?,
        module
            .function_definitions()
            .iter()
            .map(|definition| compile_function_definition(definition, context))
            .collect::<Result<Vec<_>, CompileError>>()?,
    ))
}

fn compile_calling_convention(calling_convention: CallingConvention) -> mir::ir::CallingConvention {
    match calling_convention {
        CallingConvention::Native => mir::ir::CallingConvention::Source,
        CallingConvention::C => mir::ir::CallingConvention::Target,
    }
}

fn compile_type_definition(
    type_definition: &TypeDefinition,
    context: &CompileContext,
) -> Result<mir::ir::TypeDefinition, CompileError> {
    Ok(mir::ir::TypeDefinition::new(
        type_definition.name(),
        mir::types::RecordBody::new(
            type_definition
                .fields()
                .iter()
                .map(|field| type_::compile(context, field.type_()))
                .collect::<Result<_, _>>()?,
        ),
    ))
}

fn compile_function_declaration(
    declaration: &FunctionDeclaration,
    context: &CompileContext,
) -> Result<mir::ir::FunctionDeclaration, CompileError> {
    Ok(mir::ir::FunctionDeclaration::new(
        declaration.name(),
        type_::compile_function(declaration.type_(), context)?,
    ))
}

fn compile_function_definition(
    definition: &FunctionDefinition,
    context: &CompileContext,
) -> Result<mir::ir::FunctionDefinition, CompileError> {
    let body = expression::compile(context, definition.lambda().body())?;
    let result_type = type_::compile(context, definition.lambda().result_type())?;

    Ok(if definition.lambda().arguments().is_empty() {
        mir::ir::FunctionDefinition::thunk(definition.name(), body, result_type)
    } else {
        mir::ir::FunctionDefinition::new(
            definition.name(),
            definition
                .lambda()
                .arguments()
                .iter()
                .map(|argument| -> Result<_, CompileError> {
                    Ok(mir::ir::Argument::new(
                        argument.name(),
                        type_::compile(context, argument.type_())?,
                    ))
                })
                .collect::<Result<_, _>>()?,
            body,
            result_type,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compile_configuration::COMPILE_CONFIGURATION,
        concurrency_configuration::CONCURRENCY_CONFIGURATION,
    };
    use hir::{test::ModuleFake, types};
    use mir::test::ModuleFake as _;
    use position::{test::PositionFake, Position};
    use pretty_assertions::assert_eq;

    fn compile_module(module: &Module) -> Result<mir::ir::Module, CompileError> {
        compile(
            &CompileContext::new(module, COMPILE_CONFIGURATION.clone().into()),
            module,
        )
    }

    #[test]
    fn compile_foreign_definition() {
        assert_eq!(
            compile_module(
                &Module::empty().set_definitions(vec![FunctionDefinition::new(
                    "foo",
                    "bar",
                    Lambda::new(
                        vec![Argument::new("x", types::None::new(Position::fake()))],
                        types::None::new(Position::fake()),
                        None::new(Position::fake()),
                        Position::fake(),
                    ),
                    ForeignDefinitionConfiguration::new(CallingConvention::Native).into(),
                    false,
                    Position::fake(),
                )])
            ),
            Ok(mir::ir::Module::empty()
                .set_foreign_declarations(vec![spawn_function_declaration::compile(
                    &CONCURRENCY_CONFIGURATION
                )])
                .set_foreign_definitions(vec![mir::ir::ForeignDefinition::new(
                    "foo",
                    "bar",
                    mir::ir::CallingConvention::Source
                )],)
                .set_function_definitions(vec![mir::ir::FunctionDefinition::new(
                    "foo",
                    vec![mir::ir::Argument::new("x", mir::types::Type::None)],
                    mir::ir::Expression::None,
                    mir::types::Type::None,
                )],))
        );
    }

    #[test]
    fn compile_foreign_definition_with_c_calling_convention() {
        assert_eq!(
            compile_module(
                &Module::empty().set_definitions(vec![FunctionDefinition::new(
                    "foo",
                    "bar",
                    Lambda::new(
                        vec![Argument::new("x", types::None::new(Position::fake()))],
                        types::None::new(Position::fake()),
                        None::new(Position::fake()),
                        Position::fake(),
                    ),
                    ForeignDefinitionConfiguration::new(CallingConvention::C).into(),
                    false,
                    Position::fake(),
                )])
            ),
            Ok(mir::ir::Module::empty()
                .set_foreign_declarations(vec![spawn_function_declaration::compile(
                    &CONCURRENCY_CONFIGURATION
                )])
                .set_foreign_definitions(vec![mir::ir::ForeignDefinition::new(
                    "foo",
                    "bar",
                    mir::ir::CallingConvention::Target
                )])
                .set_function_definitions(vec![mir::ir::FunctionDefinition::new(
                    "foo",
                    vec![mir::ir::Argument::new("x", mir::types::Type::None)],
                    mir::ir::Expression::None,
                    mir::types::Type::None,
                )],))
        );
    }
}
