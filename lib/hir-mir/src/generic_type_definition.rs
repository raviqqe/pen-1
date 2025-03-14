use super::{context::CompileContext, type_, CompileError};
use fnv::{FnvHashMap, FnvHashSet};
use hir::{
    analysis::{expression_visitor, union_type_member_calculator, AnalysisError},
    ir::*,
    types::Type,
};

pub fn compile(
    module: &Module,
    context: &CompileContext,
) -> Result<Vec<mir::ir::TypeDefinition>, CompileError> {
    Ok(collect_types(module, context.types())?
        .into_iter()
        .map(|type_| compile_type_definition(&type_, context))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn compile_type_definition(
    type_: &Type,
    context: &CompileContext,
) -> Result<Option<mir::ir::TypeDefinition>, CompileError> {
    Ok(match type_ {
        Type::Function(function_type) => Some(mir::ir::TypeDefinition::new(
            type_::compile_concrete_function_name(function_type, context.types())?,
            mir::types::RecordBody::new(vec![
                type_::compile_function(function_type, context)?.into()
            ]),
        )),
        Type::List(list_type) => Some(mir::ir::TypeDefinition::new(
            type_::compile_concrete_list_name(list_type, context.types())?,
            mir::types::RecordBody::new(vec![mir::types::Record::new(
                &context.configuration()?.list_type.list_type_name,
            )
            .into()]),
        )),
        Type::Map(map_type) => Some(mir::ir::TypeDefinition::new(
            type_::compile_concrete_map_name(map_type, context.types())?,
            mir::types::RecordBody::new(vec![mir::types::Record::new(
                &context.configuration()?.map_type.map_type_name,
            )
            .into()]),
        )),
        Type::Any(_)
        | Type::Boolean(_)
        | Type::String(_)
        | Type::None(_)
        | Type::Number(_)
        | Type::Record(_) => None,
        Type::Reference(_) | Type::Union(_) => unreachable!(),
    })
}

fn collect_types(
    module: &Module,
    types: &FnvHashMap<String, Type>,
) -> Result<FnvHashSet<Type>, AnalysisError> {
    let mut lower_types = FnvHashSet::default();

    // We need to visit expressions other than type coercion too because type
    // coercion might be generated just before compilation.
    expression_visitor::visit(module, |expression| match expression {
        Expression::IfList(if_) => {
            lower_types.insert(if_.type_().unwrap().clone());
        }
        Expression::IfMap(if_) => {
            lower_types.insert(if_.key_type().unwrap().clone());
            lower_types.insert(if_.value_type().unwrap().clone());
        }
        Expression::IfType(if_) => {
            lower_types.extend(
                if_.branches()
                    .iter()
                    .map(|branch| branch.type_())
                    .chain(if_.else_().and_then(|branch| branch.type_()))
                    .cloned(),
            );
        }
        Expression::List(list) => {
            lower_types.insert(list.type_().clone());
        }
        Expression::Map(map) => {
            lower_types.insert(map.key_type().clone());
            lower_types.insert(map.value_type().clone());
        }
        Expression::ListComprehension(comprehension) => {
            lower_types.insert(comprehension.input_type().unwrap().clone());
            lower_types.insert(comprehension.output_type().clone());
        }
        Expression::TypeCoercion(coercion) => {
            lower_types.insert(coercion.from().clone());
        }
        Expression::Operation(operation) => match operation {
            Operation::Equality(operation) => {
                lower_types.extend(operation.type_().cloned());
            }
            Operation::Try(operation) => {
                lower_types.extend(operation.type_().cloned());
            }
            _ => {}
        },
        _ => {}
    });

    Ok(lower_types
        .into_iter()
        .map(|type_| union_type_member_calculator::calculate(&type_, types))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hir::{
        test::{FunctionDefinitionFake, ModuleFake},
        types,
    };
    use position::{test::PositionFake, Position};

    #[test]
    fn compile_function_type_definition() {
        let function_type =
            types::Function::new(vec![], types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            function_type.clone(),
            types::None::new(Position::fake()),
            Position::fake(),
        );
        let context = CompileContext::dummy(Default::default(), Default::default());

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![Argument::new("x", function_type.clone())],
                        types::None::new(Position::fake()),
                        TypeCoercion::new(
                            function_type.clone(),
                            union_type,
                            Variable::new("x", Position::fake()),
                            Position::fake()
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_function_name(&function_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![type_::compile_function(
                    &function_type,
                    &context
                )
                .unwrap()
                .into()]),
            )])
        );
    }

    #[test]
    fn compile_list_type_definition() {
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::None::new(Position::fake()),
            Position::fake(),
        );
        let context = CompileContext::dummy(Default::default(), Default::default());

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![Argument::new("x", list_type.clone())],
                        types::None::new(Position::fake()),
                        TypeCoercion::new(
                            list_type.clone(),
                            union_type,
                            Variable::new("x", Position::fake()),
                            Position::fake()
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn compile_duplicate_list_type_definitions() {
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::None::new(Position::fake()),
            Position::fake(),
        );
        let context = CompileContext::dummy(Default::default(), Default::default());
        let definition = FunctionDefinition::fake(
            "foo",
            Lambda::new(
                vec![Argument::new("x", list_type.clone())],
                types::None::new(Position::fake()),
                TypeCoercion::new(
                    list_type.clone(),
                    union_type,
                    Variable::new("x", Position::fake()),
                    Position::fake(),
                ),
                Position::fake(),
            ),
            false,
        );

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![definition.clone(), definition]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_type_from_if_type() {
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let context = CompileContext::dummy(Default::default(), Default::default());

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![Argument::new("x", list_type.clone())],
                        types::None::new(Position::fake()),
                        IfType::new(
                            "x",
                            Variable::new("x", Position::fake()),
                            vec![IfTypeBranch::new(
                                list_type.clone(),
                                None::new(Position::fake())
                            )],
                            None,
                            Position::fake()
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_type_from_equal_operation() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::Record::new(
                &context.configuration().unwrap().error_type.error_type_name,
                Position::fake(),
            ),
            Position::fake(),
        );

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![Argument::new("x", union_type)],
                        types::Boolean::new(Position::fake()),
                        EqualityOperation::new(
                            Some(list_type.clone().into()),
                            EqualityOperator::Equal,
                            Variable::new("x", Position::fake()),
                            Variable::new("x", Position::fake()),
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_type_from_try_operation() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::Record::new(
                &context.configuration().unwrap().error_type.error_type_name,
                Position::fake(),
            ),
            Position::fake(),
        );

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![Argument::new("x", union_type)],
                        types::None::new(Position::fake()),
                        TryOperation::new(
                            Some(list_type.clone().into()),
                            Variable::new("x", Position::fake()),
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_type_from_list_literal() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::None::new(Position::fake()),
            Position::fake(),
        );

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![Argument::new("x", list_type.clone())],
                        types::None::new(Position::fake()),
                        List::new(
                            union_type,
                            vec![ListElement::Single(
                                Variable::new("x", Position::fake()).into()
                            )],
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_input_type_from_list_comprehension() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::None::new(Position::fake()),
            Position::fake(),
        );

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![],
                        types::None::new(Position::fake()),
                        ListComprehension::new(
                            Some(union_type.clone().into()),
                            types::None::new(Position::fake()),
                            None::new(Position::fake()),
                            "_",
                            List::new(union_type, vec![], Position::fake()),
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_output_type_from_list_comprehension() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let union_type = types::Union::new(
            list_type.clone(),
            types::None::new(Position::fake()),
            Position::fake(),
        );

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![],
                        types::None::new(Position::fake()),
                        ListComprehension::new(
                            Some(types::None::new(Position::fake()).into()),
                            union_type,
                            None::new(Position::fake()),
                            "_",
                            List::new(types::None::new(Position::fake()), vec![], Position::fake()),
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_type_from_if_list() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let list_type = types::List::new(types::None::new(Position::fake()), Position::fake());

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![],
                        types::None::new(Position::fake()),
                        IfList::new(
                            Some(list_type.clone().into()),
                            List::new(list_type.element().clone(), vec![], Position::fake()),
                            "x",
                            "xs",
                            None::new(Position::fake()),
                            None::new(Position::fake()),
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![mir::ir::TypeDefinition::new(
                type_::compile_concrete_list_name(&list_type, context.types()).unwrap(),
                mir::types::RecordBody::new(vec![mir::types::Record::new(
                    &context.configuration().unwrap().list_type.list_type_name
                )
                .into()]),
            )])
        );
    }

    #[test]
    fn collect_type_from_if_map() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let key_type = types::List::new(types::Number::new(Position::fake()), Position::fake());
        let value_type = types::List::new(types::None::new(Position::fake()), Position::fake());
        let map_type = types::Map::new(key_type.clone(), value_type.clone(), Position::fake());

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![],
                        types::None::new(Position::fake()),
                        IfMap::new(
                            Some(map_type.key().clone()),
                            Some(map_type.value().clone()),
                            "x",
                            Map::new(
                                map_type.key().clone(),
                                map_type.value().clone(),
                                vec![],
                                Position::fake()
                            ),
                            Number::new(42.0, Position::fake()),
                            None::new(Position::fake()),
                            None::new(Position::fake()),
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![
                mir::ir::TypeDefinition::new(
                    type_::compile_concrete_list_name(&key_type, context.types()).unwrap(),
                    mir::types::RecordBody::new(vec![mir::types::Record::new(
                        &context.configuration().unwrap().list_type.list_type_name
                    )
                    .into()]),
                ),
                mir::ir::TypeDefinition::new(
                    type_::compile_concrete_list_name(&value_type, context.types()).unwrap(),
                    mir::types::RecordBody::new(vec![mir::types::Record::new(
                        &context.configuration().unwrap().list_type.list_type_name
                    )
                    .into()]),
                )
            ])
        );
    }

    #[test]
    fn collect_type_from_map_literal() {
        let context = CompileContext::dummy(Default::default(), Default::default());
        let key_type = types::List::new(types::Number::new(Position::fake()), Position::fake());
        let value_type = types::List::new(types::None::new(Position::fake()), Position::fake());

        assert_eq!(
            compile(
                &Module::empty().set_definitions(vec![FunctionDefinition::fake(
                    "foo",
                    Lambda::new(
                        vec![],
                        types::None::new(Position::fake()),
                        Map::new(
                            key_type.clone(),
                            value_type.clone(),
                            vec![],
                            Position::fake(),
                        ),
                        Position::fake(),
                    ),
                    false,
                )]),
                &context,
            ),
            Ok(vec![
                mir::ir::TypeDefinition::new(
                    type_::compile_concrete_list_name(&key_type, context.types()).unwrap(),
                    mir::types::RecordBody::new(vec![mir::types::Record::new(
                        &context.configuration().unwrap().list_type.list_type_name
                    )
                    .into()]),
                ),
                mir::ir::TypeDefinition::new(
                    type_::compile_concrete_list_name(&value_type, context.types()).unwrap(),
                    mir::types::RecordBody::new(vec![mir::types::Record::new(
                        &context.configuration().unwrap().list_type.list_type_name
                    )
                    .into()]),
                )
            ])
        );
    }
}
