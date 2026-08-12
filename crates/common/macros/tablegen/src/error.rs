use {proc_macro2::Span, syn::Error};

pub fn err(span: Span, msg: impl std::fmt::Display) -> Error {
    Error::new(span, msg)
}

pub fn combine_errors(errors: Vec<Error>) -> Result<(), Error> {
    errors
        .into_iter()
        .reduce(|mut acc, e| {
            acc.combine(e);
            acc
        })
        .map_or(Ok(()), Err)
}
