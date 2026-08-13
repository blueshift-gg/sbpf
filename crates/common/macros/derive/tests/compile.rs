#[cfg(test)]
mod tests {

    #[test]
    fn compile_fail() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/fixtures/compile_fail.rs");
    }
}
