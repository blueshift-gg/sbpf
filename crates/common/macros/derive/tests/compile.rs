#[cfg(test)]
mod tests {
    #[test]
    fn compile_pass() {
        let t = trybuild::TestCases::new();
        t.pass("tests/fixtures/compile_pass.rs");
    }

    #[test]
    fn compile_fail() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/fixtures/compile_fail.rs");
    }
}
