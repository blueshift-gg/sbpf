#[macro_export]
macro_rules! define_compile_errors {
    (
        $(
            $variant:ident {
                error = $error_msg:literal,
                label = $label_msg:literal,
                fields = { $( $field_name:ident : $field_ty:ty ),* $(,)? }
            }
        ),* $(,)?
    ) => {
        #[derive(Debug, thiserror::Error)]
        pub enum CompileError {
            $(
                #[error($error_msg)]
                $variant { $( $field_name: $field_ty ),*, custom_label: Option<String> }
            ),*
        }

        impl CompileError {
            pub fn label(&self) -> &str {
                match self {
                    $(
                        Self::$variant { custom_label, .. } => custom_label.as_deref().unwrap_or($label_msg),
                    )*
                }
            }

            pub fn span(&self) -> &Range<usize> {
                match self {
                    $(
                        Self::$variant { span, .. } => span,
                    )*
                }
            }
        }
    };
}

// TODO: make it a hyper link
#[macro_export]
macro_rules! bug {
    ($($arg:tt)*) => {{
        eprintln!(
            "\n{}\n{}",
            "Thanks for abusing the compiler <3 you've hunted a bug!",
            format!("Please file a bug report at: {}", "https://github.com/blueshift-gg/sbpf/issues/new")
        );

        panic!("{}", format!("Internal error: {}\n", format!($($arg)*)));
    }};
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    #[test]
    fn test_define_compile_errors_macro() {
        define_compile_errors! {
            TestError1 {
                error = "Test error 1",
                label = "test label 1",
                fields = { span: Range<usize> }
            },
            TestError2 {
                error = "Test error 2",
                label = "test label 2",
                fields = { span: Range<usize>, message: String }
            }
        }

        // Test creating errors
        let err1 = CompileError::TestError1 {
            span: 0..10,
            custom_label: None,
        };
        assert_eq!(err1.label(), "test label 1");
        assert_eq!(err1.span(), &(0..10));
        assert_eq!(err1.to_string(), "Test error 1");

        let err2 = CompileError::TestError2 {
            span: 5..15,
            message: "custom message".to_string(),
            custom_label: Some("custom".to_string()),
        };
        assert_eq!(err2.label(), "custom");
        assert_eq!(err2.span(), &(5..15));
    }
}
