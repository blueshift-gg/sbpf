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
