use super::{context::AnalysisContext, AnalysisError};
use crate::{
    analysis::{expression_visitor, type_canonicalizer, type_subsumption_checker},
    ir::*,
};

pub fn validate(context: &AnalysisContext, module: &Module) -> Result<(), AnalysisError> {
    if let Ok(error_type) = context.error_type() {
        for expression in collect_expressions(module) {
            if let Expression::Let(let_) = expression {
                let expression = let_.bound_expression();
                let type_ = let_
                    .type_()
                    .ok_or_else(|| AnalysisError::TypeNotInferred(expression.position().clone()))?;

                if let_.name().is_none()
                    && !type_canonicalizer::canonicalize(type_, context.types())?.is_any()
                    && type_subsumption_checker::check(error_type, type_, context.types())?
                {
                    return Err(AnalysisError::UnusedErrorValue(
                        expression.position().clone(),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn collect_expressions(module: &Module) -> Vec<Expression> {
    let mut expressions = vec![];

    expression_visitor::visit(module, |expression| {
        if matches!(expression, Expression::Let(_)) {
            expressions.push(expression.clone())
        }
    });

    expressions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::type_collector,
        test::{FunctionDefinitionFake, ModuleFake, TypeDefinitionFake},
        types::{self, Type},
    };
    use once_cell::sync::Lazy;
    use position::{test::PositionFake, Position};

    const ERROR_TYPE_NAME: &str = "error";

    static ERROR_TYPE: Lazy<Type> =
        Lazy::new(|| types::Reference::new(ERROR_TYPE_NAME, Position::fake()).into());
    static ERROR_TYPE_DEFINITION: Lazy<TypeDefinition> =
        Lazy::new(|| TypeDefinition::fake(ERROR_TYPE_NAME, vec![], false, false, false));

    fn validate_module(module: &Module) -> Result<(), AnalysisError> {
        validate(
            &AnalysisContext::new(
                type_collector::collect(module),
                type_collector::collect_records(module),
                Some(types::Record::new("error", Position::fake()).into()),
            ),
            module,
        )
    }

    #[test]
    fn validate_none_type() {
        assert_eq!(
            validate_module(
                &Module::empty()
                    .set_type_definitions(vec![ERROR_TYPE_DEFINITION.clone()])
                    .set_definitions(vec![FunctionDefinition::fake(
                        "f",
                        Lambda::new(
                            vec![],
                            types::None::new(Position::fake()),
                            Let::new(
                                None,
                                Some(types::None::new(Position::fake()).into()),
                                None::new(Position::fake()),
                                None::new(Position::fake()),
                                Position::fake(),
                            ),
                            Position::fake(),
                        ),
                        false,
                    )]),
            ),
            Ok(()),
        );
    }

    #[test]
    fn validate_any_type() {
        assert_eq!(
            validate_module(
                &Module::empty()
                    .set_type_definitions(vec![ERROR_TYPE_DEFINITION.clone()])
                    .set_definitions(vec![FunctionDefinition::fake(
                        "f",
                        Lambda::new(
                            vec![],
                            types::None::new(Position::fake()),
                            Let::new(
                                None,
                                Some(types::Any::new(Position::fake()).into()),
                                None::new(Position::fake()),
                                None::new(Position::fake()),
                                Position::fake(),
                            ),
                            Position::fake(),
                        ),
                        false,
                    )]),
            ),
            Ok(()),
        );
    }

    #[test]
    fn fail_to_validate_error_type() {
        assert_eq!(
            validate_module(
                &Module::empty()
                    .set_type_definitions(vec![ERROR_TYPE_DEFINITION.clone()])
                    .set_definitions(vec![FunctionDefinition::fake(
                        "f",
                        Lambda::new(
                            vec![Argument::new("x", ERROR_TYPE.clone())],
                            types::None::new(Position::fake()),
                            Let::new(
                                None,
                                Some(ERROR_TYPE.clone()),
                                Variable::new("x", Position::fake()),
                                None::new(Position::fake()),
                                Position::fake(),
                            ),
                            Position::fake(),
                        ),
                        false,
                    )]),
            ),
            Err(AnalysisError::UnusedErrorValue(Position::fake())),
        );
    }

    #[test]
    fn fail_to_validate_result_type() {
        let result_type = types::Union::new(
            types::None::new(Position::fake()),
            ERROR_TYPE.clone(),
            Position::fake(),
        );

        assert_eq!(
            validate_module(
                &Module::empty()
                    .set_type_definitions(vec![ERROR_TYPE_DEFINITION.clone()])
                    .set_definitions(vec![FunctionDefinition::fake(
                        "f",
                        Lambda::new(
                            vec![Argument::new("x", result_type.clone())],
                            types::None::new(Position::fake()),
                            Let::new(
                                None,
                                Some(result_type.into()),
                                Variable::new("x", Position::fake()),
                                None::new(Position::fake()),
                                Position::fake(),
                            ),
                            Position::fake(),
                        ),
                        false,
                    )]),
            ),
            Err(AnalysisError::UnusedErrorValue(Position::fake())),
        );
    }
}
