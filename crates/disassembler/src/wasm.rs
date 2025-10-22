use {
    crate::program::Program as EBPFProgram,
    serde::{Deserialize, Serialize},
    serde_wasm_bindgen::to_value,
    wasm_bindgen::prelude::*,
};

#[wasm_bindgen]
#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    inner: EBPFProgram,
}

#[wasm_bindgen]
impl Program {
    #[wasm_bindgen]
    pub fn from_bytes(b: &[u8]) -> Result<Program, JsValue> {
        EBPFProgram::from_bytes(b)
            .map(|program| Program { inner: program })
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    #[wasm_bindgen]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
